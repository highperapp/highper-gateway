# Highper Gateway - AWS Variables
# Separated variables file for better modularity

variable "region" {
  description = "AWS region"
  type        = string
  default     = "us-east-1"
}

variable "environment" {
  description = "Environment name (dev, staging, prod)"
  type        = string
  default     = "prod"

  validation {
    condition     = contains(["dev", "staging", "prod"], var.environment)
    error_message = "Environment must be one of: dev, staging, prod."
  }
}

variable "instance_type" {
  description = "EC2 instance type - compute optimized recommended for io_uring"
  type        = string
  default     = "c6i.large"
}

variable "instance_count" {
  description = "Number of instances in Auto Scaling Group"
  type        = number
  default     = 2

  validation {
    condition     = var.instance_count >= 1 && var.instance_count <= 100
    error_message = "Instance count must be between 1 and 100."
  }
}

variable "use_case" {
  description = "Deployment use case (01-15)"
  type        = string
  default     = "02"

  validation {
    condition     = can(regex("^(0[1-9]|1[0-5])$", var.use_case))
    error_message = "Use case must be between 01 and 15."
  }
}

variable "enable_https" {
  description = "Enable HTTPS/TLS termination"
  type        = bool
  default     = true
}

variable "domain_name" {
  description = "Domain name for the gateway (optional)"
  type        = string
  default     = ""
}

variable "ssh_key_name" {
  description = "SSH key pair name for EC2 instances"
  type        = string
}

variable "allowed_cidr_blocks" {
  description = "CIDR blocks allowed to access the gateway"
  type        = list(string)
  default     = ["0.0.0.0/0"]
}

variable "vpc_cidr" {
  description = "CIDR block for the VPC"
  type        = string
  default     = "10.0.0.0/16"
}

variable "enable_ecr" {
  description = "Create ECR repository for container deployments"
  type        = bool
  default     = true
}

variable "enable_cloudwatch" {
  description = "Enable CloudWatch logging and metrics"
  type        = bool
  default     = true
}

variable "tags" {
  description = "Additional tags to apply to resources"
  type        = map(string)
  default     = {}
}
