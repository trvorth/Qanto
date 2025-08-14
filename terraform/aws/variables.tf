# ============================================================================
# General Configuration
# ============================================================================

variable "aws_region" {
  description = "AWS region where resources will be created"
  type        = string
  default     = "us-east-1"
}

variable "environment" {
  description = "Environment name (e.g., production, staging)"
  type        = string
  default     = "production"
}

variable "cluster_name" {
  description = "Name of the EKS cluster"
  type        = string
  default     = "qanto-production"
}

# ============================================================================
# Network Configuration
# ============================================================================

variable "vpc_cidr" {
  description = "CIDR block for VPC"
  type        = string
  default     = "10.0.0.0/16"
}

variable "private_subnet_cidrs" {
  description = "CIDR blocks for private subnets"
  type        = list(string)
  default     = ["10.0.1.0/24", "10.0.2.0/24", "10.0.3.0/24"]
}

variable "public_subnet_cidrs" {
  description = "CIDR blocks for public subnets"
  type        = list(string)
  default     = ["10.0.101.0/24", "10.0.102.0/24", "10.0.103.0/24"]
}

variable "admin_cidr_blocks" {
  description = "CIDR blocks allowed to access admin endpoints"
  type        = list(string)
  default     = ["0.0.0.0/0"]  # Change this to restrict access
}

# ============================================================================
# EKS Configuration
# ============================================================================

variable "kubernetes_version" {
  description = "Kubernetes version to use for the EKS cluster"
  type        = string
  default     = "1.28"
}

# ============================================================================
# Blockchain Node Pool Configuration
# ============================================================================

variable "blockchain_instance_types" {
  description = "EC2 instance types for blockchain nodes"
  type        = list(string)
  default     = ["t3.large"]  # 2 vCPU, 8 GB RAM
}

variable "blockchain_min_nodes" {
  description = "Minimum number of nodes in blockchain pool"
  type        = number
  default     = 1
}

variable "blockchain_max_nodes" {
  description = "Maximum number of nodes in blockchain pool"
  type        = number
  default     = 3
}

variable "blockchain_desired_nodes" {
  description = "Desired number of nodes in blockchain pool"
  type        = number
  default     = 2
}

# ============================================================================
# Web Node Pool Configuration
# ============================================================================

variable "web_instance_types" {
  description = "EC2 instance types for web nodes"
  type        = list(string)
  default     = ["t3.medium"]  # 2 vCPU, 4 GB RAM
}

variable "web_min_nodes" {
  description = "Minimum number of nodes in web pool"
  type        = number
  default     = 2
}

variable "web_max_nodes" {
  description = "Maximum number of nodes in web pool"
  type        = number
  default     = 5
}

variable "web_desired_nodes" {
  description = "Desired number of nodes in web pool"
  type        = number
  default     = 3
}

# ============================================================================
# Domain Configuration
# ============================================================================

variable "domain_name" {
  description = "Domain name for the Qanto website"
  type        = string
  default     = "qanto.org"
}

variable "route53_zone_id" {
  description = "Route53 Hosted Zone ID for the domain (leave empty to skip DNS setup)"
  type        = string
  default     = ""
}

# ============================================================================
# Container Registry
# ============================================================================

variable "ecr_repository_names" {
  description = "Names of ECR repositories to create"
  type        = list(string)
  default     = ["qanto-node", "qanto-website"]
}
