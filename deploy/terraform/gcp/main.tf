# Highper Gateway - GCP Infrastructure
# Terraform module for deploying Highper Gateway on Google Cloud Platform

terraform {
  required_version = ">= 1.0"
  required_providers {
    google = {
      source  = "hashicorp/google"
      version = "~> 5.0"
    }
  }
}

# Variables
variable "project_id" {
  description = "GCP project ID"
  type        = string
}

variable "region" {
  description = "GCP region"
  type        = string
  default     = "us-central1"
}

variable "zone" {
  description = "GCP zone"
  type        = string
  default     = "us-central1-a"
}

variable "environment" {
  description = "Environment name (dev, staging, prod)"
  type        = string
  default     = "prod"
}

variable "machine_type" {
  description = "GCE machine type"
  type        = string
  default     = "c2-standard-4"  # Compute optimized for io_uring
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

variable "allowed_source_ranges" {
  description = "CIDR ranges allowed to access the gateway"
  type        = list(string)
  default     = ["0.0.0.0/0"]
}

# Local variables
locals {
  name_prefix = "highper-gateway-${var.environment}"

  use_case_ports = {
    "01" = [80, 443, 3306, 5432]
    "02" = [80, 8080]
    "03" = [443, 8443]
    "04" = [8080, 443]
    "05" = [443]
    "06" = [80, 443, 8080]
    "07" = [9090, 443]
    "08" = [3306, 5432, 6379]
    "09" = [443]
    "10" = [80, 443, 8080, 9090]
    "11" = [80, 443]
    "12" = [8080, 8500]
    "13" = [4000, 443]
    "14" = [80, 443]
    "15" = [80, 443]
  }

  selected_ports = lookup(local.use_case_ports, var.use_case, [80, 443])

  common_labels = {
    project     = "highper-gateway"
    environment = var.environment
    use-case    = var.use_case
    managed-by  = "terraform"
  }
}

# Provider
provider "google" {
  project = var.project_id
  region  = var.region
}

# VPC Network
resource "google_compute_network" "main" {
  name                    = "${local.name_prefix}-vpc"
  auto_create_subnetworks = false
}

# Subnet
resource "google_compute_subnetwork" "main" {
  name          = "${local.name_prefix}-subnet"
  ip_cidr_range = "10.0.0.0/24"
  region        = var.region
  network       = google_compute_network.main.id

  log_config {
    aggregation_interval = "INTERVAL_10_MIN"
    flow_sampling        = 0.5
    metadata             = "INCLUDE_ALL_METADATA"
  }
}

# Firewall - Allow selected ports
resource "google_compute_firewall" "allow_gateway" {
  name    = "${local.name_prefix}-allow-gateway"
  network = google_compute_network.main.name

  allow {
    protocol = "tcp"
    ports    = [for p in local.selected_ports : tostring(p)]
  }

  # UDP for HTTP/3 QUIC (use case 05)
  dynamic "allow" {
    for_each = var.use_case == "05" ? [1] : []
    content {
      protocol = "udp"
      ports    = ["443"]
    }
  }

  source_ranges = var.allowed_source_ranges
  target_tags   = ["highper-gateway"]
}

# Firewall - Allow SSH
resource "google_compute_firewall" "allow_ssh" {
  name    = "${local.name_prefix}-allow-ssh"
  network = google_compute_network.main.name

  allow {
    protocol = "tcp"
    ports    = ["22"]
  }

  source_ranges = var.allowed_source_ranges
  target_tags   = ["highper-gateway"]
}

# Firewall - Allow health checks
resource "google_compute_firewall" "allow_health_check" {
  name    = "${local.name_prefix}-allow-health-check"
  network = google_compute_network.main.name

  allow {
    protocol = "tcp"
    ports    = ["8081"]
  }

  source_ranges = ["130.211.0.0/22", "35.191.0.0/16"]
  target_tags   = ["highper-gateway"]
}

# Service Account
resource "google_service_account" "gateway" {
  account_id   = "${local.name_prefix}-sa"
  display_name = "Highper Gateway Service Account"
}

# Instance Template
resource "google_compute_instance_template" "gateway" {
  name_prefix  = "${local.name_prefix}-"
  machine_type = var.machine_type
  region       = var.region

  disk {
    source_image = "ubuntu-os-cloud/ubuntu-2204-lts"
    auto_delete  = true
    boot         = true
    disk_type    = "pd-ssd"
    disk_size_gb = 20
  }

  network_interface {
    network    = google_compute_network.main.id
    subnetwork = google_compute_subnetwork.main.id

    access_config {
      # Ephemeral public IP
    }
  }

  service_account {
    email  = google_service_account.gateway.email
    scopes = ["cloud-platform"]
  }

  metadata_startup_script = templatefile("${path.module}/startup.sh.tpl", {
    use_case    = var.use_case
    environment = var.environment
  })

  tags = ["highper-gateway"]

  labels = local.common_labels

  lifecycle {
    create_before_destroy = true
  }
}

# Health Check
resource "google_compute_health_check" "gateway" {
  name               = "${local.name_prefix}-health-check"
  check_interval_sec = 10
  timeout_sec        = 5

  tcp_health_check {
    port = local.selected_ports[0]
  }
}

# Instance Group Manager
resource "google_compute_region_instance_group_manager" "gateway" {
  name   = "${local.name_prefix}-mig"
  region = var.region

  base_instance_name = "${local.name_prefix}-instance"

  version {
    instance_template = google_compute_instance_template.gateway.id
  }

  target_size = var.instance_count

  named_port {
    name = "gateway"
    port = local.selected_ports[0]
  }

  auto_healing_policies {
    health_check      = google_compute_health_check.gateway.id
    initial_delay_sec = 300
  }
}

# Backend Service
resource "google_compute_region_backend_service" "gateway" {
  name                  = "${local.name_prefix}-backend"
  region                = var.region
  load_balancing_scheme = "EXTERNAL"
  protocol              = "TCP"

  backend {
    group = google_compute_region_instance_group_manager.gateway.instance_group
  }

  health_checks = [google_compute_health_check.gateway.id]
}

# Regional External TCP Load Balancer
resource "google_compute_forwarding_rule" "gateway" {
  for_each = toset([for p in local.selected_ports : tostring(p)])

  name                  = "${local.name_prefix}-forwarding-${each.value}"
  region                = var.region
  load_balancing_scheme = "EXTERNAL"
  port_range            = each.value
  backend_service       = google_compute_region_backend_service.gateway.id
}

# Artifact Registry (for container deployments)
resource "google_artifact_registry_repository" "gateway" {
  location      = var.region
  repository_id = "highper-gateway"
  format        = "DOCKER"

  labels = local.common_labels
}

# Outputs
output "vpc_id" {
  description = "VPC ID"
  value       = google_compute_network.main.id
}

output "load_balancer_ips" {
  description = "Load balancer IP addresses"
  value       = { for k, v in google_compute_forwarding_rule.gateway : k => v.ip_address }
}

output "artifact_registry_url" {
  description = "Artifact Registry URL"
  value       = "${var.region}-docker.pkg.dev/${var.project_id}/${google_artifact_registry_repository.gateway.repository_id}"
}

output "instance_group" {
  description = "Instance group URL"
  value       = google_compute_region_instance_group_manager.gateway.instance_group
}

output "use_case_ports" {
  description = "Ports configured for this use case"
  value       = local.selected_ports
}
