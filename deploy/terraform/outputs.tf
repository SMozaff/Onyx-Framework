output "cluster_name" {
  value = module.eks.cluster_name
}

output "cluster_endpoint" {
  value     = module.eks.cluster_endpoint
  sensitive = true
}

output "database_endpoint" {
  value     = module.database.db_instance_endpoint
  sensitive = true
}

output "database_password" {
  value     = random_password.database.result
  sensitive = true
}

output "backup_bucket" {
  value = aws_s3_bucket.backups.id
}

output "ecr_repositories" {
  value = { for name, repository in aws_ecr_repository.service : name => repository.repository_url }
}
