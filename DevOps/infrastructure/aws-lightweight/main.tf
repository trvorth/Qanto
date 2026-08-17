# Terraform AWS Infrastructure - Sub-$10/Month Qanto Node
# Target Instance: AWS Graviton2 t4g.micro (2 vCPU, 1 GB RAM)
# Target Cost: ~$4.45 - $6.93 / month (with 10GB GP3 EBS Storage)

terraform {
  required_version = ">= 1.5.0"
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }
}

provider "aws" {
  region = var.aws_region
}

variable "aws_region" {
  type    = string
  default = "us-east-1"
}

variable "environment" {
  type    = string
  default = "production"
}

# Fetch latest Amazon Linux 2023 ARM64 AMI
data "aws_ami" "amazon_linux_arm64" {
  most_recent = true
  owners      = ["137112412989"] # Amazon

  filter {
    name   = "name"
    values = ["al2023-ami-2023.*-arm64"]
  }

  filter {
    name   = "architecture"
    values = ["arm64"]
  }
}

# VPC Security Group for Qanto P2P, Web API, RPC
resource "aws_security_group" "qanto_sg" {
  name        = "qanto-lightweight-node-sg"
  description = "Security group for Qanto sub-10-dollar node"

  # P2P Mesh Gossip
  ingress {
    from_port   = 30303
    to_port     = 30303
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  # REST API / Health
  ingress {
    from_port   = 8081
    to_port     = 8081
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  # gRPC RPC Endpoint (Restricted / Public)
  ingress {
    from_port   = 50051
    to_port     = 50051
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  # Outbound Traffic
  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }
}

# AWS Graviton2 t4g.micro Instance (~$6.13/mo On-Demand, ~$3.65/mo Savings Plan)
resource "aws_instance" "qanto_node" {
  ami           = data.aws_ami.amazon_linux_arm64.id
  instance_type = "t4g.micro"

  vpc_security_group_ids = [aws_security_group.qanto_sg.id]

  # 10 GB GP3 EBS Disk (~$0.80/mo)
  root_block_device {
    volume_size           = 10
    volume_type           = "gp3"
    iops                  = 3000
    throughput            = 125
    delete_on_termination = false
  }

  user_data = <<-EOF
              #!/bin/bash
              dnf update -y
              dnf install -y docker
              systemctl enable --now docker
              docker run -d \
                --name qanto-node \
                --restart always \
                -p 30303:30303 \
                -p 8081:8081 \
                -p 50051:50051 \
                -e QANTO_PROFILE=aws-lightweight \
                qanto/qanto-node:latest
              EOF

  tags = {
    Name        = "Qanto-Node-AWS-Sub10USD"
    Environment = var.environment
    CostTarget  = "Sub10USDPerMonth"
  }
}

output "instance_public_ip" {
  value       = aws_instance.qanto_node.public_ip
  description = "Public IP address of the AWS Qanto Node"
}

output "estimated_monthly_cost" {
  value       = "~$4.45 - $6.93 USD / month (Compute + 10GB GP3 Storage)"
  description = "Estimated total monthly AWS running cost"
}
