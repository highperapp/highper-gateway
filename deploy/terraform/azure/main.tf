# Highper Gateway - Azure Infrastructure
# Terraform module for deploying Highper Gateway on Microsoft Azure

terraform {
  required_version = ">= 1.0"
  required_providers {
    azurerm = {
      source  = "hashicorp/azurerm"
      version = "~> 3.0"
    }
  }
}

# Variables
variable "location" {
  description = "Azure region"
  type        = string
  default     = "East US"
}

variable "environment" {
  description = "Environment name (dev, staging, prod)"
  type        = string
  default     = "prod"
}

variable "vm_size" {
  description = "Azure VM size"
  type        = string
  default     = "Standard_F4s_v2"  # Compute optimized for io_uring
}

variable "instance_count" {
  description = "Number of VM instances"
  type        = number
  default     = 2
}

variable "use_case" {
  description = "Deployment use case (01-15)"
  type        = string
  default     = "02"
}

variable "admin_username" {
  description = "Admin username for VMs"
  type        = string
  default     = "azureuser"
}

variable "ssh_public_key" {
  description = "SSH public key for VM access"
  type        = string
}

variable "allowed_source_addresses" {
  description = "IP addresses allowed to access the gateway"
  type        = list(string)
  default     = ["*"]
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

  common_tags = {
    Project     = "highper-gateway"
    Environment = var.environment
    UseCase     = var.use_case
    ManagedBy   = "terraform"
  }
}

# Provider
provider "azurerm" {
  features {}
}

# Resource Group
resource "azurerm_resource_group" "main" {
  name     = "${local.name_prefix}-rg"
  location = var.location

  tags = local.common_tags
}

# Virtual Network
resource "azurerm_virtual_network" "main" {
  name                = "${local.name_prefix}-vnet"
  address_space       = ["10.0.0.0/16"]
  location            = azurerm_resource_group.main.location
  resource_group_name = azurerm_resource_group.main.name

  tags = local.common_tags
}

# Subnet
resource "azurerm_subnet" "main" {
  name                 = "${local.name_prefix}-subnet"
  resource_group_name  = azurerm_resource_group.main.name
  virtual_network_name = azurerm_virtual_network.main.name
  address_prefixes     = ["10.0.1.0/24"]
}

# Network Security Group
resource "azurerm_network_security_group" "main" {
  name                = "${local.name_prefix}-nsg"
  location            = azurerm_resource_group.main.location
  resource_group_name = azurerm_resource_group.main.name

  # SSH access
  security_rule {
    name                       = "SSH"
    priority                   = 100
    direction                  = "Inbound"
    access                     = "Allow"
    protocol                   = "Tcp"
    source_port_range          = "*"
    destination_port_range     = "22"
    source_address_prefixes    = var.allowed_source_addresses
    destination_address_prefix = "*"
  }

  tags = local.common_tags
}

# Dynamic security rules for use case ports
resource "azurerm_network_security_rule" "gateway_ports" {
  for_each = toset([for p in local.selected_ports : tostring(p)])

  name                        = "Gateway-Port-${each.value}"
  priority                    = 200 + index(local.selected_ports, tonumber(each.value))
  direction                   = "Inbound"
  access                      = "Allow"
  protocol                    = "Tcp"
  source_port_range           = "*"
  destination_port_range      = each.value
  source_address_prefixes     = var.allowed_source_addresses
  destination_address_prefix  = "*"
  resource_group_name         = azurerm_resource_group.main.name
  network_security_group_name = azurerm_network_security_group.main.name
}

# HTTP/3 QUIC UDP rule (use case 05)
resource "azurerm_network_security_rule" "quic" {
  count = var.use_case == "05" ? 1 : 0

  name                        = "QUIC-UDP"
  priority                    = 300
  direction                   = "Inbound"
  access                      = "Allow"
  protocol                    = "Udp"
  source_port_range           = "*"
  destination_port_range      = "443"
  source_address_prefixes     = var.allowed_source_addresses
  destination_address_prefix  = "*"
  resource_group_name         = azurerm_resource_group.main.name
  network_security_group_name = azurerm_network_security_group.main.name
}

# Subnet NSG Association
resource "azurerm_subnet_network_security_group_association" "main" {
  subnet_id                 = azurerm_subnet.main.id
  network_security_group_id = azurerm_network_security_group.main.id
}

# Public IP for Load Balancer
resource "azurerm_public_ip" "lb" {
  name                = "${local.name_prefix}-lb-pip"
  location            = azurerm_resource_group.main.location
  resource_group_name = azurerm_resource_group.main.name
  allocation_method   = "Static"
  sku                 = "Standard"

  tags = local.common_tags
}

# Load Balancer
resource "azurerm_lb" "main" {
  name                = "${local.name_prefix}-lb"
  location            = azurerm_resource_group.main.location
  resource_group_name = azurerm_resource_group.main.name
  sku                 = "Standard"

  frontend_ip_configuration {
    name                 = "PublicIPAddress"
    public_ip_address_id = azurerm_public_ip.lb.id
  }

  tags = local.common_tags
}

# Backend Pool
resource "azurerm_lb_backend_address_pool" "main" {
  loadbalancer_id = azurerm_lb.main.id
  name            = "${local.name_prefix}-backend-pool"
}

# Health Probe
resource "azurerm_lb_probe" "main" {
  loadbalancer_id = azurerm_lb.main.id
  name            = "${local.name_prefix}-health-probe"
  protocol        = "Tcp"
  port            = local.selected_ports[0]
  interval_in_seconds = 10
  number_of_probes = 2
}

# Load Balancer Rules
resource "azurerm_lb_rule" "gateway" {
  for_each = toset([for p in local.selected_ports : tostring(p)])

  loadbalancer_id                = azurerm_lb.main.id
  name                           = "Rule-${each.value}"
  protocol                       = "Tcp"
  frontend_port                  = tonumber(each.value)
  backend_port                   = tonumber(each.value)
  frontend_ip_configuration_name = "PublicIPAddress"
  backend_address_pool_ids       = [azurerm_lb_backend_address_pool.main.id]
  probe_id                       = azurerm_lb_probe.main.id
  enable_tcp_reset               = true
}

# Virtual Machine Scale Set
resource "azurerm_linux_virtual_machine_scale_set" "main" {
  name                = "${local.name_prefix}-vmss"
  resource_group_name = azurerm_resource_group.main.name
  location            = azurerm_resource_group.main.location
  sku                 = var.vm_size
  instances           = var.instance_count
  admin_username      = var.admin_username

  admin_ssh_key {
    username   = var.admin_username
    public_key = var.ssh_public_key
  }

  source_image_reference {
    publisher = "Canonical"
    offer     = "0001-com-ubuntu-server-jammy"
    sku       = "22_04-lts-gen2"
    version   = "latest"
  }

  os_disk {
    storage_account_type = "Premium_LRS"
    caching              = "ReadWrite"
    disk_size_gb         = 30
  }

  network_interface {
    name    = "nic"
    primary = true

    ip_configuration {
      name                                   = "internal"
      primary                                = true
      subnet_id                              = azurerm_subnet.main.id
      load_balancer_backend_address_pool_ids = [azurerm_lb_backend_address_pool.main.id]
    }
  }

  custom_data = base64encode(templatefile("${path.module}/cloud-init.yaml.tpl", {
    use_case    = var.use_case
    environment = var.environment
  }))

  tags = local.common_tags
}

# Container Registry
resource "azurerm_container_registry" "main" {
  name                = replace("${local.name_prefix}acr", "-", "")
  resource_group_name = azurerm_resource_group.main.name
  location            = azurerm_resource_group.main.location
  sku                 = "Basic"
  admin_enabled       = false

  tags = local.common_tags
}

# Outputs
output "resource_group_name" {
  description = "Resource group name"
  value       = azurerm_resource_group.main.name
}

output "load_balancer_ip" {
  description = "Load balancer public IP"
  value       = azurerm_public_ip.lb.ip_address
}

output "container_registry_url" {
  description = "Container registry URL"
  value       = azurerm_container_registry.main.login_server
}

output "vmss_name" {
  description = "Virtual Machine Scale Set name"
  value       = azurerm_linux_virtual_machine_scale_set.main.name
}

output "use_case_ports" {
  description = "Ports configured for this use case"
  value       = local.selected_ports
}
