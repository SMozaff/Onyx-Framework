variable "project_name" {
  description = "Resource-name prefix."
  type        = string
  default     = "onyx"
}

variable "environment" {
  description = "Deployment environment."
  type        = string
  default     = "production"
  validation {
    condition     = contains(["staging", "production"], var.environment)
    error_message = "environment must be staging or production"
  }
}

variable "aws_region" {
  type    = string
  default = "eu-central-1"
}

variable "vpc_cidr" {
  type    = string
  default = "10.40.0.0/16"
}

variable "db_instance_class" {
  type    = string
  default = "db.r6g.large"
}

variable "db_allocated_storage_gb" {
  type    = number
  default = 100
}

variable "kubernetes_version" {
  type    = string
  default = "1.30"
}

variable "tags" {
  type    = map(string)
  default = {}
}
