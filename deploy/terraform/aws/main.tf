# Highper Gateway - AWS Infrastructure
# Terraform module for deploying Highper Gateway on AWS

terraform {
  required_version = ">= 1.0"
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }
}

# Variables
variable "region" {
  description = "AWS region"
  type        = string
  default     = "us-east-1"
}

variable "environment" {
  description = "Environment name (dev, staging, prod)"
  type        = string
  default     = "prod"
}

variable "instance_type" {
  description = "EC2 instance type"
  type        = string
  default     = "c6i.large"  # Compute optimized for io_uring workloads
}

variable "instance_count" {
  description = "Number of instances"
  type        = number
  default     = 2
}

variable "use_case" {
  description = "Deployment use case (01-15)"
  type        = string
  default     = "02"
}

variable "enable_https" {
  description = "Enable HTTPS/TLS termination"
  type        = bool
  default     = true
}

variable "domain_name" {
  description = "Domain name for the gateway"
  type        = string
  default     = ""
}

variable "ssh_key_name" {
  description = "SSH key pair name"
  type        = string
}

variable "allowed_cidr_blocks" {
  description = "CIDR blocks allowed to access the gateway"
  type        = list(string)
  default     = ["0.0.0.0/0"]
}

# Local variables
locals {
  name_prefix = "highper-gateway-${var.environment}"

  # Use case port mappings
  use_case_ports = {
    "01" = [80, 443, 3306, 5432]      # TCP Proxy
    "02" = [80, 8080]                  # HTTP Load Balancer
    "03" = [443, 8443]                 # HTTPS/TLS
    "04" = [8080, 443]                 # API Gateway
    "05" = [443]                       # HTTP/3 QUIC (UDP)
    "06" = [80, 443, 8080]             # WebSocket
    "07" = [9090, 443]                 # gRPC
    "08" = [3306, 5432, 6379]          # Database LB
    "09" = [443]                       # WAF + mTLS
    "10" = [80, 443, 8080, 9090]       # Hybrid
    "11" = [80, 443]                   # CDN Edge
    "12" = [8080, 8500]                # Discovery
    "13" = [4000, 443]                 # GraphQL
    "14" = [80, 443]                   # Static + PHP-FPM
    "15" = [80, 443]                   # Geo LB
  }

  selected_ports = lookup(local.use_case_ports, var.use_case, [80, 443])

  common_tags = {
    Project     = "highper-gateway"
    Environment = var.environment
    UseCase     = var.use_case
    ManagedBy   = "terraform"
  }
}

# Provider configuration
provider "aws" {
  region = var.region
}

# Data sources
data "aws_availability_zones" "available" {
  state = "available"
}

data "aws_ami" "ubuntu" {
  most_recent = true
  owners      = ["099720109477"]  # Canonical

  filter {
    name   = "name"
    values = ["ubuntu/images/hvm-ssd/ubuntu-jammy-22.04-amd64-server-*"]
  }

  filter {
    name   = "virtualization-type"
    values = ["hvm"]
  }
}

# VPC
resource "aws_vpc" "main" {
  cidr_block           = "10.0.0.0/16"
  enable_dns_hostnames = true
  enable_dns_support   = true

  tags = merge(local.common_tags, {
    Name = "${local.name_prefix}-vpc"
  })
}

# Internet Gateway
resource "aws_internet_gateway" "main" {
  vpc_id = aws_vpc.main.id

  tags = merge(local.common_tags, {
    Name = "${local.name_prefix}-igw"
  })
}

# Subnets (Public)
resource "aws_subnet" "public" {
  count                   = min(length(data.aws_availability_zones.available.names), 3)
  vpc_id                  = aws_vpc.main.id
  cidr_block              = "10.0.${count.index + 1}.0/24"
  availability_zone       = data.aws_availability_zones.available.names[count.index]
  map_public_ip_on_launch = true

  tags = merge(local.common_tags, {
    Name = "${local.name_prefix}-public-${count.index + 1}"
    Type = "public"
  })
}

# Route Table
resource "aws_route_table" "public" {
  vpc_id = aws_vpc.main.id

  route {
    cidr_block = "0.0.0.0/0"
    gateway_id = aws_internet_gateway.main.id
  }

  tags = merge(local.common_tags, {
    Name = "${local.name_prefix}-public-rt"
  })
}

resource "aws_route_table_association" "public" {
  count          = length(aws_subnet.public)
  subnet_id      = aws_subnet.public[count.index].id
  route_table_id = aws_route_table.public.id
}

# Security Group
resource "aws_security_group" "gateway" {
  name        = "${local.name_prefix}-sg"
  description = "Security group for Highper Gateway"
  vpc_id      = aws_vpc.main.id

  # Dynamic ingress rules based on use case
  dynamic "ingress" {
    for_each = local.selected_ports
    content {
      from_port   = ingress.value
      to_port     = ingress.value
      protocol    = "tcp"
      cidr_blocks = var.allowed_cidr_blocks
      description = "Port ${ingress.value} for use case ${var.use_case}"
    }
  }

  # HTTP/3 QUIC (UDP) - only for use case 05
  dynamic "ingress" {
    for_each = var.use_case == "05" ? [443] : []
    content {
      from_port   = ingress.value
      to_port     = ingress.value
      protocol    = "udp"
      cidr_blocks = var.allowed_cidr_blocks
      description = "HTTP/3 QUIC UDP"
    }
  }

  # SSH access
  ingress {
    from_port   = 22
    to_port     = 22
    protocol    = "tcp"
    cidr_blocks = var.allowed_cidr_blocks
    description = "SSH access"
  }

  # Egress
  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }

  tags = merge(local.common_tags, {
    Name = "${local.name_prefix}-sg"
  })
}

# IAM Role for EC2
resource "aws_iam_role" "gateway" {
  name = "${local.name_prefix}-role"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Action = "sts:AssumeRole"
        Effect = "Allow"
        Principal = {
          Service = "ec2.amazonaws.com"
        }
      }
    ]
  })

  tags = local.common_tags
}

resource "aws_iam_role_policy_attachment" "ssm" {
  role       = aws_iam_role.gateway.name
  policy_arn = "arn:aws:iam::aws:policy/AmazonSSMManagedInstanceCore"
}

resource "aws_iam_instance_profile" "gateway" {
  name = "${local.name_prefix}-profile"
  role = aws_iam_role.gateway.name
}

# Launch Template
resource "aws_launch_template" "gateway" {
  name_prefix   = "${local.name_prefix}-"
  image_id      = data.aws_ami.ubuntu.id
  instance_type = var.instance_type
  key_name      = var.ssh_key_name

  iam_instance_profile {
    name = aws_iam_instance_profile.gateway.name
  }

  network_interfaces {
    associate_public_ip_address = true
    security_groups             = [aws_security_group.gateway.id]
  }

  # User data for initial setup
  user_data = base64encode(templatefile("${path.module}/user_data.sh.tpl", {
    use_case    = var.use_case
    environment = var.environment
  }))

  # EBS optimization
  ebs_optimized = true

  block_device_mappings {
    device_name = "/dev/sda1"
    ebs {
      volume_size           = 20
      volume_type           = "gp3"
      delete_on_termination = true
      encrypted             = true
    }
  }

  tag_specifications {
    resource_type = "instance"
    tags = merge(local.common_tags, {
      Name = "${local.name_prefix}-instance"
    })
  }

  tags = local.common_tags
}

# Auto Scaling Group
resource "aws_autoscaling_group" "gateway" {
  name                = "${local.name_prefix}-asg"
  desired_capacity    = var.instance_count
  min_size            = 1
  max_size            = var.instance_count * 2
  vpc_zone_identifier = aws_subnet.public[*].id
  health_check_type   = "ELB"

  launch_template {
    id      = aws_launch_template.gateway.id
    version = "$Latest"
  }

  target_group_arns = [aws_lb_target_group.gateway.arn]

  tag {
    key                 = "Name"
    value               = "${local.name_prefix}-instance"
    propagate_at_launch = true
  }

  dynamic "tag" {
    for_each = local.common_tags
    content {
      key                 = tag.key
      value               = tag.value
      propagate_at_launch = true
    }
  }
}

# Network Load Balancer (for L4 use cases)
resource "aws_lb" "gateway" {
  name               = "${local.name_prefix}-nlb"
  internal           = false
  load_balancer_type = "network"
  subnets            = aws_subnet.public[*].id

  enable_cross_zone_load_balancing = true

  tags = local.common_tags
}

# Target Group
resource "aws_lb_target_group" "gateway" {
  name        = "${local.name_prefix}-tg"
  port        = local.selected_ports[0]
  protocol    = "TCP"
  vpc_id      = aws_vpc.main.id
  target_type = "instance"

  health_check {
    enabled             = true
    healthy_threshold   = 2
    unhealthy_threshold = 2
    interval            = 10
    protocol            = "TCP"
    port                = "traffic-port"
  }

  tags = local.common_tags
}

# Listener
resource "aws_lb_listener" "gateway" {
  for_each          = toset([for p in local.selected_ports : tostring(p)])
  load_balancer_arn = aws_lb.gateway.arn
  port              = each.value
  protocol          = "TCP"

  default_action {
    type             = "forward"
    target_group_arn = aws_lb_target_group.gateway.arn
  }
}

# ECR Repository (for container deployments)
resource "aws_ecr_repository" "gateway" {
  name                 = "highper-gateway"
  image_tag_mutability = "MUTABLE"

  image_scanning_configuration {
    scan_on_push = true
  }

  encryption_configuration {
    encryption_type = "AES256"
  }

  tags = local.common_tags
}

# ECR Lifecycle Policy
resource "aws_ecr_lifecycle_policy" "gateway" {
  repository = aws_ecr_repository.gateway.name

  policy = jsonencode({
    rules = [
      {
        rulePriority = 1
        description  = "Keep last 10 tagged images"
        selection = {
          tagStatus     = "tagged"
          tagPrefixList = ["v"]
          countType     = "imageCountMoreThan"
          countNumber   = 10
        }
        action = {
          type = "expire"
        }
      },
      {
        rulePriority = 2
        description  = "Expire untagged images older than 7 days"
        selection = {
          tagStatus   = "untagged"
          countType   = "sinceImagePushed"
          countUnit   = "days"
          countNumber = 7
        }
        action = {
          type = "expire"
        }
      }
    ]
  })
}

# Outputs
output "vpc_id" {
  description = "VPC ID"
  value       = aws_vpc.main.id
}

output "nlb_dns_name" {
  description = "Network Load Balancer DNS name"
  value       = aws_lb.gateway.dns_name
}

output "ecr_repository_url" {
  description = "ECR repository URL"
  value       = aws_ecr_repository.gateway.repository_url
}

output "security_group_id" {
  description = "Security group ID"
  value       = aws_security_group.gateway.id
}

output "asg_name" {
  description = "Auto Scaling Group name"
  value       = aws_autoscaling_group.gateway.name
}

output "use_case_ports" {
  description = "Ports configured for this use case"
  value       = local.selected_ports
}
