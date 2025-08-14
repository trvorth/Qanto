terraform {
  required_version = ">= 1.0"
  
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = "~> 2.23"
    }
    helm = {
      source  = "hashicorp/helm"
      version = "~> 2.11"
    }
  }
  
  # backend "s3" {
  #   # Configure this with your own S3 bucket for state storage
  #   # bucket = "qanto-terraform-state"
  #   # key    = "production/terraform.tfstate"
  #   # region = "us-east-1"
  # }
}

# ============================================================================
# Provider Configuration
# ============================================================================

provider "aws" {
  region = var.aws_region
  
  default_tags {
    tags = {
      Project     = "Qanto"
      Environment = var.environment
      ManagedBy   = "Terraform"
    }
  }
}

# Temporarily commented out to avoid circular dependency during cluster creation
# provider "kubernetes" {
#   host                   = module.eks.cluster_endpoint
#   cluster_ca_certificate = base64decode(module.eks.cluster_certificate_authority_data)
#   
#   exec {
#     api_version = "client.authentication.k8s.io/v1beta1"
#     command     = "aws"
#     args        = ["eks", "get-token", "--cluster-name", module.eks.cluster_id]
#   }
# }
# 
# provider "helm" {
#   kubernetes {
#     host                   = module.eks.cluster_endpoint
#     cluster_ca_certificate = base64decode(module.eks.cluster_certificate_authority_data)
#     
#     exec {
#       api_version = "client.authentication.k8s.io/v1beta1"
#       command     = "aws"
#       args        = ["eks", "get-token", "--cluster-name", module.eks.cluster_id]
#     }
#   }
# }

# ============================================================================
# VPC and Networking
# ============================================================================

module "vpc" {
  source  = "terraform-aws-modules/vpc/aws"
  version = "~> 5.0"
  
  name = "${var.cluster_name}-vpc"
  cidr = var.vpc_cidr
  
  azs             = data.aws_availability_zones.available.names
  private_subnets = var.private_subnet_cidrs
  public_subnets  = var.public_subnet_cidrs
  
  enable_nat_gateway   = true
  single_nat_gateway   = false  # High availability
  enable_dns_hostnames = true
  enable_dns_support   = true
  
  # Kubernetes tags for subnet discovery
  public_subnet_tags = {
    "kubernetes.io/role/elb"                    = "1"
    "kubernetes.io/cluster/${var.cluster_name}" = "shared"
  }
  
  private_subnet_tags = {
    "kubernetes.io/role/internal-elb"           = "1"
    "kubernetes.io/cluster/${var.cluster_name}" = "shared"
  }
  
  tags = {
    Name = "${var.cluster_name}-vpc"
  }
}

# ============================================================================
# Static IP Addresses (Elastic IPs)
# ============================================================================

resource "aws_eip" "boot_node" {
  domain = "vpc"
  
  tags = {
    Name    = "${var.cluster_name}-boot-node-eip"
    Purpose = "Qanto Boot Node P2P"
  }
}

resource "aws_eip" "website" {
  domain = "vpc"
  
  tags = {
    Name    = "${var.cluster_name}-website-eip"
    Purpose = "Qanto Website Ingress"
  }
}

# ============================================================================
# Security Groups
# ============================================================================

resource "aws_security_group" "boot_node" {
  name_prefix = "${var.cluster_name}-boot-node-"
  vpc_id      = module.vpc.vpc_id
  description = "Security group for Qanto boot node"
  
  # P2P port for Qanto
  ingress {
    from_port   = 30333
    to_port     = 30333
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
    description = "Qanto P2P networking"
  }
  
  # RPC port for Qanto
  ingress {
    from_port   = 9933
    to_port     = 9933
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
    description = "Qanto RPC access"
  }
  
  # WebSocket port for Qanto
  ingress {
    from_port   = 9944
    to_port     = 9944
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
    description = "Qanto WebSocket access"
  }
  
  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
    description = "Allow all outbound"
  }
  
  tags = {
    Name = "${var.cluster_name}-boot-node-sg"
  }
}

# ============================================================================
# EKS Cluster
# ============================================================================

module "eks" {
  source  = "terraform-aws-modules/eks/aws"
  version = "~> 19.0"
  
  cluster_name    = var.cluster_name
  cluster_version = var.kubernetes_version
  
  vpc_id     = module.vpc.vpc_id
  subnet_ids = module.vpc.private_subnets
  
  # Enable IRSA (IAM Roles for Service Accounts)
  enable_irsa = true
  
  # Cluster endpoint access
  cluster_endpoint_public_access  = true
  cluster_endpoint_private_access = true
  cluster_endpoint_public_access_cidrs = var.admin_cidr_blocks
  
  # Cluster addons
  cluster_addons = {
    coredns = {
      most_recent = true
    }
    kube-proxy = {
      most_recent = true
    }
    vpc-cni = {
      most_recent = true
    }
    aws-ebs-csi-driver = {
      most_recent = true
    }
  }
  
  # Node groups
  eks_managed_node_groups = {
    # Blockchain node pool - for running boot nodes
    blockchain = {
      name           = "qanto-blockchain"
      instance_types = var.blockchain_instance_types

      # Short IAM role name to satisfy AWS limits
      iam_role_use_name_prefix = false
      iam_role_name            = "qanto-prod-ng-block"
      
      min_size     = var.blockchain_min_nodes
      max_size     = var.blockchain_max_nodes
      desired_size = var.blockchain_desired_nodes
      
      disk_size = 100  # GB for blockchain data
      
      labels = {
        workload = "blockchain"
        pool     = "blockchain"
      }
      
        taints = [
          {
            key    = "blockchain"
            value  = "true"
            effect = "NO_SCHEDULE"
          }
        ]
      
      tags = {
        Purpose = "Blockchain nodes"
      }
    }
    
    # Web node pool - for running website and general workloads
    web = {
      name           = "qanto-web"
      instance_types = var.web_instance_types

      # Short IAM role name to satisfy AWS limits
      iam_role_use_name_prefix = false
      iam_role_name            = "qanto-prod-ng-web"
      
      min_size     = var.web_min_nodes
      max_size     = var.web_max_nodes
      desired_size = var.web_desired_nodes
      
      disk_size = 50  # GB
      
      labels = {
        workload = "web"
        pool     = "web"
      }
      
      tags = {
        Purpose = "Web applications"
      }
    }
  }
  
  tags = {
    Name = var.cluster_name
  }
}

# ============================================================================
# IAM Roles for Service Accounts
# ============================================================================

# Role for AWS Load Balancer Controller
module "lb_controller_irsa" {
  source  = "terraform-aws-modules/iam/aws//modules/iam-role-for-service-accounts-eks"
  version = "~> 5.0"
  
  role_name = "${var.cluster_name}-aws-lb-controller"
  
  attach_load_balancer_controller_policy = true
  
  oidc_providers = {
    main = {
      provider_arn               = module.eks.oidc_provider_arn
      namespace_service_accounts = ["kube-system:aws-load-balancer-controller"]
    }
  }
}

# Role for External DNS
module "external_dns_irsa" {
  source  = "terraform-aws-modules/iam/aws//modules/iam-role-for-service-accounts-eks"
  version = "~> 5.0"
  
  role_name = "${var.cluster_name}-external-dns"
  
  attach_external_dns_policy = true
  external_dns_hosted_zone_arns = ["arn:aws:route53:::hostedzone/*"]
  
  oidc_providers = {
    main = {
      provider_arn               = module.eks.oidc_provider_arn
      namespace_service_accounts = ["kube-system:external-dns"]
    }
  }
}

# Role for Cert Manager
module "cert_manager_irsa" {
  source  = "terraform-aws-modules/iam/aws//modules/iam-role-for-service-accounts-eks"
  version = "~> 5.0"
  
  role_name = "${var.cluster_name}-cert-manager"
  
  attach_cert_manager_policy = true
  cert_manager_hosted_zone_arns = ["arn:aws:route53:::hostedzone/*"]
  
  oidc_providers = {
    main = {
      provider_arn               = module.eks.oidc_provider_arn
      namespace_service_accounts = ["cert-manager:cert-manager"]
    }
  }
}

# ============================================================================
# EBS StorageClass for Persistent Volumes
# ============================================================================

# Note: StorageClass will be created via Helm charts to avoid circular dependency
# resource "kubernetes_storage_class" "gp3" {
#   depends_on = [module.eks]
#   
#   metadata {
#     name = "gp3"
#     annotations = {
#       "storageclass.kubernetes.io/is-default-class" = "true"
#     }
#   }
#   
#   storage_provisioner    = "ebs.csi.aws.com"
#   reclaim_policy        = "Delete"
#   allow_volume_expansion = true
#   volume_binding_mode   = "WaitForFirstConsumer"
#   
#   parameters = {
#     type = "gp3"
#     iops = "3000"
#     throughput = "125"
#     encrypted = "true"
#   }
# }

# ============================================================================
# Data Sources
# ============================================================================

data "aws_availability_zones" "available" {
  state = "available"
}

data "aws_caller_identity" "current" {}

# ============================================================================
# Outputs
# ============================================================================

output "cluster_endpoint" {
  description = "Endpoint for EKS control plane"
  value       = module.eks.cluster_endpoint
  sensitive   = true
}

output "cluster_name" {
  description = "Kubernetes cluster name"
  value       = module.eks.cluster_id
}

output "cluster_security_group_id" {
  description = "Security group ID attached to the EKS cluster"
  value       = module.eks.cluster_security_group_id
}

output "boot_node_eip" {
  description = "Elastic IP address for the boot node"
  value       = aws_eip.boot_node.public_ip
}

output "website_eip" {
  description = "Elastic IP address for the website"
  value       = aws_eip.website.public_ip
}

output "vpc_id" {
  description = "VPC ID"
  value       = module.vpc.vpc_id
}

output "lb_controller_role_arn" {
  description = "IAM role ARN for AWS Load Balancer Controller"
  value       = module.lb_controller_irsa.iam_role_arn
}

output "external_dns_role_arn" {
  description = "IAM role ARN for External DNS"
  value       = module.external_dns_irsa.iam_role_arn
}

output "cert_manager_role_arn" {
  description = "IAM role ARN for Cert Manager"
  value       = module.cert_manager_irsa.iam_role_arn
}
