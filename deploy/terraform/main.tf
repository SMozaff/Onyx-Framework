terraform {
  required_version = ">= 1.7.0"
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.60"
    }
    random = {
      source  = "hashicorp/random"
      version = "~> 3.6"
    }
  }
}

provider "aws" {
  region = var.aws_region
  default_tags {
    tags = merge({
      Project     = var.project_name
      Environment = var.environment
      ManagedBy   = "terraform"
      RTO          = "lt-1-hour"
      RPO          = "lt-5-minutes"
    }, var.tags)
  }
}

data "aws_availability_zones" "available" {
  state = "available"
}

locals {
  name = "${var.project_name}-${var.environment}"
  azs  = slice(data.aws_availability_zones.available.names, 0, 3)
}

module "vpc" {
  source  = "terraform-aws-modules/vpc/aws"
  version = "5.13.0"

  name = local.name
  cidr = var.vpc_cidr
  azs  = local.azs
  private_subnets = [for index, _ in local.azs : cidrsubnet(var.vpc_cidr, 4, index)]
  public_subnets  = [for index, _ in local.azs : cidrsubnet(var.vpc_cidr, 4, index + 8)]

  enable_nat_gateway     = true
  single_nat_gateway     = var.environment == "staging"
  one_nat_gateway_per_az = var.environment == "production"
  enable_dns_hostnames   = true

  public_subnet_tags = {
    "kubernetes.io/role/elb" = 1
  }
  private_subnet_tags = {
    "kubernetes.io/role/internal-elb" = 1
  }
}

module "eks" {
  source  = "terraform-aws-modules/eks/aws"
  version = "20.24.0"

  cluster_name    = local.name
  cluster_version = var.kubernetes_version
  cluster_endpoint_public_access = true
  vpc_id     = module.vpc.vpc_id
  subnet_ids = module.vpc.private_subnets

  enable_cluster_creator_admin_permissions = true
  cluster_enabled_log_types = ["api", "audit", "authenticator", "controllerManager", "scheduler"]

  eks_managed_node_groups = {
    core = {
      min_size       = var.environment == "production" ? 3 : 2
      max_size       = var.environment == "production" ? 12 : 5
      desired_size   = var.environment == "production" ? 3 : 2
      instance_types = ["m7g.large"]
      capacity_type  = "ON_DEMAND"
    }
  }
}

resource "random_password" "database" {
  length           = 32
  special          = true
  override_special = "!#$%&*()-_=+[]{}<>:?"
}

resource "aws_security_group" "database" {
  name        = "${local.name}-postgres"
  description = "PostgreSQL access from ONYX EKS workers"
  vpc_id      = module.vpc.vpc_id

  ingress {
    protocol        = "tcp"
    from_port       = 5432
    to_port         = 5432
    security_groups = [module.eks.node_security_group_id]
  }
  egress {
    protocol    = "-1"
    from_port   = 0
    to_port     = 0
    cidr_blocks = ["0.0.0.0/0"]
  }
}

module "database" {
  source  = "terraform-aws-modules/rds/aws"
  version = "6.8.0"

  identifier = local.name
  engine               = "postgres"
  engine_version       = "16.3"
  family               = "postgres16"
  major_engine_version = "16"
  instance_class       = var.db_instance_class

  allocated_storage     = var.db_allocated_storage_gb
  max_allocated_storage = var.db_allocated_storage_gb * 5
  storage_encrypted     = true
  kms_key_id            = aws_kms_key.onyx.arn

  db_name  = "onyx"
  username = "onyx"
  password = random_password.database.result
  port     = 5432

  multi_az               = var.environment == "production"
  vpc_security_group_ids = [aws_security_group.database.id]
  create_db_subnet_group = true
  subnet_ids             = module.vpc.private_subnets

  backup_retention_period = 35
  backup_window           = "01:00-01:30"
  maintenance_window      = "Sun:02:00-Sun:03:00"
  deletion_protection     = var.environment == "production"
  skip_final_snapshot     = false
  final_snapshot_identifier_prefix = "${local.name}-final"
  copy_tags_to_snapshot   = true
  performance_insights_enabled          = true
  performance_insights_retention_period = 7
  monitoring_interval                   = 60
  create_monitoring_role                = true

  parameters = [
    { name = "rds.logical_replication", value = "1" },
    { name = "log_min_duration_statement", value = "500" }
  ]
}

resource "aws_kms_key" "onyx" {
  description             = "ONYX data encryption"
  deletion_window_in_days = 30
  enable_key_rotation     = true
}

resource "aws_s3_bucket" "backups" {
  bucket = "${local.name}-backup-${data.aws_caller_identity.current.account_id}"
}

data "aws_caller_identity" "current" {}

resource "aws_s3_bucket_versioning" "backups" {
  bucket = aws_s3_bucket.backups.id
  versioning_configuration {
    status = "Enabled"
  }
}

resource "aws_s3_bucket_public_access_block" "backups" {
  bucket = aws_s3_bucket.backups.id

  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}

resource "aws_s3_bucket_server_side_encryption_configuration" "backups" {
  bucket = aws_s3_bucket.backups.id
  rule {
    apply_server_side_encryption_by_default {
      kms_master_key_id = aws_kms_key.onyx.arn
      sse_algorithm     = "aws:kms"
    }
  }
}

resource "aws_s3_bucket_lifecycle_configuration" "backups" {
  bucket = aws_s3_bucket.backups.id
  rule {
    id     = "retention"
    status = "Enabled"

    filter {}

    transition {
      days          = 30
      storage_class = "STANDARD_IA"
    }

    transition {
      days          = 90
      storage_class = "GLACIER"
    }

    expiration {
      days = 365
    }
  }
}

resource "aws_ecr_repository" "service" {
  for_each = toset(["api-server", "worker", "sync-agent", "migration-tool"])

  name                 = "${local.name}/${each.key}"
  image_tag_mutability = "IMMUTABLE"

  image_scanning_configuration {
    scan_on_push = true
  }

  encryption_configuration {
    encryption_type = "KMS"
    kms_key          = aws_kms_key.onyx.arn
  }
}

resource "aws_ecr_lifecycle_policy" "service" {
  for_each   = aws_ecr_repository.service
  repository = each.value.name
  policy = jsonencode({
    rules = [{
      rulePriority = 1
      description  = "Retain the newest 100 release images"
      selection = {
        tagStatus   = "any"
        countType   = "imageCountMoreThan"
        countNumber = 100
      }
      action = { type = "expire" }
    }]
  })
}

resource "aws_cloudwatch_log_group" "application" {
  name              = "/onyx/${var.environment}/application"
  retention_in_days = var.environment == "production" ? 90 : 30
  kms_key_id        = aws_kms_key.onyx.arn
}
