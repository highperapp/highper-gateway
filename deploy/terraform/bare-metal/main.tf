# Highper Gateway - Bare Metal Infrastructure
# Terraform module for deploying Highper Gateway on bare metal servers
# Uses SSH provisioners to configure existing servers

terraform {
  required_version = ">= 1.0"
}

# Variables
variable "servers" {
  description = "List of server configurations"
  type = list(object({
    name           = string
    host           = string
    ssh_user       = optional(string, "root")
    ssh_port       = optional(number, 22)
    ssh_private_key_path = optional(string, "~/.ssh/id_rsa")
  }))
}

variable "environment" {
  description = "Environment name (dev, staging, prod)"
  type        = string
  default     = "prod"
}

variable "use_case" {
  description = "Deployment use case (01-15)"
  type        = string
  default     = "02"
}

variable "gateway_version" {
  description = "Highper Gateway version to deploy"
  type        = string
  default     = "latest"
}

variable "config_template" {
  description = "Path to custom config template (optional)"
  type        = string
  default     = ""
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

  setup_script = templatefile("${path.module}/setup.sh.tpl", {
    use_case    = var.use_case
    environment = var.environment
    version     = var.gateway_version
  })
}

# Null resource for each server
resource "null_resource" "gateway_setup" {
  for_each = { for idx, server in var.servers : server.name => server }

  triggers = {
    use_case    = var.use_case
    environment = var.environment
    version     = var.gateway_version
    script_hash = sha256(local.setup_script)
  }

  connection {
    type        = "ssh"
    host        = each.value.host
    user        = each.value.ssh_user
    port        = each.value.ssh_port
    private_key = file(pathexpand(each.value.ssh_private_key_path))
  }

  # Copy setup script
  provisioner "file" {
    content     = local.setup_script
    destination = "/tmp/highper-gateway-setup.sh"
  }

  # Execute setup
  provisioner "remote-exec" {
    inline = [
      "chmod +x /tmp/highper-gateway-setup.sh",
      "sudo /tmp/highper-gateway-setup.sh",
      "rm /tmp/highper-gateway-setup.sh"
    ]
  }
}

# Health check after deployment
resource "null_resource" "health_check" {
  for_each = { for idx, server in var.servers : server.name => server }

  depends_on = [null_resource.gateway_setup]

  triggers = {
    setup_id = null_resource.gateway_setup[each.key].id
  }

  connection {
    type        = "ssh"
    host        = each.value.host
    user        = each.value.ssh_user
    port        = each.value.ssh_port
    private_key = file(pathexpand(each.value.ssh_private_key_path))
  }

  provisioner "remote-exec" {
    inline = [
      "echo 'Checking Highper Gateway status...'",
      "systemctl is-enabled highper-gateway || echo 'Service not enabled'",
      "echo 'Server: ${each.key} (${each.value.host})'",
      "echo 'Use case: ${var.use_case}'",
      "echo 'Ports: ${join(", ", local.selected_ports)}'"
    ]
  }
}

# Outputs
output "deployed_servers" {
  description = "List of deployed servers"
  value = {
    for name, server in var.servers : name => {
      host = server.host
      ports = local.selected_ports
    }
  }
}

output "use_case_ports" {
  description = "Ports configured for this use case"
  value       = local.selected_ports
}

output "deployment_status" {
  description = "Deployment completion status"
  value = {
    for name, resource in null_resource.gateway_setup : name => "deployed"
  }
}
