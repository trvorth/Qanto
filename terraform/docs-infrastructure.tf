# Qanto Documentation Portal Infrastructure
# Deploys docs.qanto.org with CloudFront, S3, and Route53

# Configure the AWS Provider
terraform {
  required_version = ">= 1.0"
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }
  
  # Store state in S3
  backend "s3" {
    bucket         = "qanto-terraform-state"
    key            = "docs-infrastructure/terraform.tfstate"
    region         = "us-east-1"
    encrypt        = true
    dynamodb_table = "terraform-state-locks"
  }
}

provider "aws" {
  region = var.aws_region
  
  default_tags {
    tags = {
      Project     = "qanto-docs"
      Environment = var.environment
      Terraform   = "true"
      Team        = "infrastructure"
    }
  }
}

# Data sources
data "aws_route53_zone" "qanto_org" {
  name         = "qanto.org"
  private_zone = false
}

data "aws_acm_certificate" "qanto_wildcard" {
  domain      = "*.qanto.org"
  statuses    = ["ISSUED"]
  most_recent = true
  
  # ACM certificates for CloudFront must be in us-east-1
  provider = aws.us_east_1
}

# AWS Provider for us-east-1 (required for CloudFront certificates)
provider "aws" {
  alias  = "us_east_1"
  region = "us-east-1"
  
  default_tags {
    tags = {
      Project     = "qanto-docs"
      Environment = var.environment
      Terraform   = "true"
      Team        = "infrastructure"
    }
  }
}

# Variables
variable "environment" {
  description = "Environment name"
  type        = string
  default     = "production"
}

variable "aws_region" {
  description = "AWS region"
  type        = string
  default     = "us-west-2"
}

variable "domain_name" {
  description = "Domain name for the docs site"
  type        = string
  default     = "docs.qanto.org"
}

variable "node_version" {
  description = "Node.js version for building docs"
  type        = string
  default     = "18.x"
}

# S3 bucket for hosting the static site
resource "aws_s3_bucket" "docs_bucket" {
  bucket = "qanto-docs-${var.environment}-${random_id.bucket_suffix.hex}"
}

resource "random_id" "bucket_suffix" {
  byte_length = 4
}

# S3 bucket public access settings
resource "aws_s3_bucket_public_access_block" "docs_bucket_pab" {
  bucket = aws_s3_bucket.docs_bucket.id

  block_public_acls       = false
  block_public_policy     = false
  ignore_public_acls      = false
  restrict_public_buckets = false
}

# S3 bucket website configuration
resource "aws_s3_bucket_website_configuration" "docs_bucket_website" {
  bucket = aws_s3_bucket.docs_bucket.id

  index_document {
    suffix = "index.html"
  }

  error_document {
    key = "404.html"
  }
}

# S3 bucket policy for public read access
resource "aws_s3_bucket_policy" "docs_bucket_policy" {
  bucket = aws_s3_bucket.docs_bucket.id
  depends_on = [aws_s3_bucket_public_access_block.docs_bucket_pab]

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Sid       = "PublicReadGetObject"
        Effect    = "Allow"
        Principal = "*"
        Action    = "s3:GetObject"
        Resource  = "${aws_s3_bucket.docs_bucket.arn}/*"
      },
    ]
  })
}

# S3 bucket versioning
resource "aws_s3_bucket_versioning" "docs_bucket_versioning" {
  bucket = aws_s3_bucket.docs_bucket.id
  versioning_configuration {
    status = "Enabled"
  }
}

# S3 bucket server-side encryption
resource "aws_s3_bucket_server_side_encryption_configuration" "docs_bucket_encryption" {
  bucket = aws_s3_bucket.docs_bucket.id

  rule {
    apply_server_side_encryption_by_default {
      sse_algorithm = "AES256"
    }
  }
}

# CloudFront Origin Access Control
resource "aws_cloudfront_origin_access_control" "docs_oac" {
  name                              = "qanto-docs-oac"
  description                       = "Origin Access Control for Qanto Documentation"
  origin_access_control_origin_type = "s3"
  signing_behavior                  = "always"
  signing_protocol                  = "sigv4"
}

# CloudFront distribution
resource "aws_cloudfront_distribution" "docs_distribution" {
  origin {
    domain_name              = aws_s3_bucket.docs_bucket.bucket_regional_domain_name
    origin_access_control_id = aws_cloudfront_origin_access_control.docs_oac.id
    origin_id                = "S3-${aws_s3_bucket.docs_bucket.bucket}"
  }

  enabled             = true
  is_ipv6_enabled     = true
  comment             = "Qanto Documentation Portal"
  default_root_object = "index.html"

  aliases = [var.domain_name]

  default_cache_behavior {
    allowed_methods  = ["DELETE", "GET", "HEAD", "OPTIONS", "PATCH", "POST", "PUT"]
    cached_methods   = ["GET", "HEAD"]
    target_origin_id = "S3-${aws_s3_bucket.docs_bucket.bucket}"

    forwarded_values {
      query_string = false

      cookies {
        forward = "none"
      }
    }

    viewer_protocol_policy = "redirect-to-https"
    min_ttl                = 0
    default_ttl            = 3600
    max_ttl                = 86400
    compress               = true
  }

  # Cache behavior for API endpoints (if any)
  ordered_cache_behavior {
    path_pattern     = "/api/*"
    allowed_methods  = ["GET", "HEAD", "OPTIONS"]
    cached_methods   = ["GET", "HEAD"]
    target_origin_id = "S3-${aws_s3_bucket.docs_bucket.bucket}"

    forwarded_values {
      query_string = true
      headers      = ["Authorization", "CloudFront-Forwarded-Proto"]

      cookies {
        forward = "none"
      }
    }

    viewer_protocol_policy = "redirect-to-https"
    min_ttl                = 0
    default_ttl            = 300
    max_ttl                = 3600
    compress               = true
  }

  # Cache behavior for assets (CSS, JS, images)
  ordered_cache_behavior {
    path_pattern     = "/assets/*"
    allowed_methods  = ["GET", "HEAD"]
    cached_methods   = ["GET", "HEAD"]
    target_origin_id = "S3-${aws_s3_bucket.docs_bucket.bucket}"

    forwarded_values {
      query_string = false

      cookies {
        forward = "none"
      }
    }

    viewer_protocol_policy = "redirect-to-https"
    min_ttl                = 0
    default_ttl            = 31536000  # 1 year
    max_ttl                = 31536000
    compress               = true
  }

  price_class = "PriceClass_All"

  restrictions {
    geo_restriction {
      restriction_type = "none"
    }
  }

  viewer_certificate {
    acm_certificate_arn      = data.aws_acm_certificate.qanto_wildcard.arn
    ssl_support_method       = "sni-only"
    minimum_protocol_version = "TLSv1.2_2021"
  }

  custom_error_response {
    error_code            = 404
    error_caching_min_ttl = 300
    response_code         = 404
    response_page_path    = "/404.html"
  }

  custom_error_response {
    error_code            = 403
    error_caching_min_ttl = 300
    response_code         = 404
    response_page_path    = "/404.html"
  }

  tags = {
    Name = "qanto-docs-cloudfront"
  }
}

# Route53 record
resource "aws_route53_record" "docs_record" {
  zone_id = data.aws_route53_zone.qanto_org.zone_id
  name    = var.domain_name
  type    = "A"

  alias {
    name                   = aws_cloudfront_distribution.docs_distribution.domain_name
    zone_id                = aws_cloudfront_distribution.docs_distribution.hosted_zone_id
    evaluate_target_health = false
  }
}

# Route53 record for IPv6
resource "aws_route53_record" "docs_record_ipv6" {
  zone_id = data.aws_route53_zone.qanto_org.zone_id
  name    = var.domain_name
  type    = "AAAA"

  alias {
    name                   = aws_cloudfront_distribution.docs_distribution.domain_name
    zone_id                = aws_cloudfront_distribution.docs_distribution.hosted_zone_id
    evaluate_target_health = false
  }
}

# IAM role for GitHub Actions deployment
resource "aws_iam_role" "github_actions_role" {
  name = "qanto-docs-github-actions"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Action = "sts:AssumeRoleWithWebIdentity"
        Effect = "Allow"
        Principal = {
          Federated = aws_iam_openid_connect_provider.github_actions.arn
        }
        Condition = {
          StringEquals = {
            "token.actions.githubusercontent.com:aud" = "sts.amazonaws.com"
          }
          StringLike = {
            "token.actions.githubusercontent.com:sub" = "repo:trvworth/qanto:*"
          }
        }
      }
    ]
  })
}

# IAM policy for GitHub Actions
resource "aws_iam_role_policy" "github_actions_policy" {
  name = "qanto-docs-deploy-policy"
  role = aws_iam_role.github_actions_role.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Action = [
          "s3:PutObject",
          "s3:PutObjectAcl",
          "s3:GetObject",
          "s3:DeleteObject",
          "s3:ListBucket"
        ]
        Resource = [
          aws_s3_bucket.docs_bucket.arn,
          "${aws_s3_bucket.docs_bucket.arn}/*"
        ]
      },
      {
        Effect = "Allow"
        Action = [
          "cloudfront:CreateInvalidation",
          "cloudfront:GetInvalidation",
          "cloudfront:ListInvalidations"
        ]
        Resource = aws_cloudfront_distribution.docs_distribution.arn
      }
    ]
  })
}

# OIDC provider for GitHub Actions
resource "aws_iam_openid_connect_provider" "github_actions" {
  url = "https://token.actions.githubusercontent.com"

  client_id_list = ["sts.amazonaws.com"]

  thumbprint_list = [
    "6938fd4d98bab03faadb97b34396831e3780aea1",
    "1c58a3a8518e8759bf075b76b750d4f2df264fcd"
  ]
}

# S3 bucket for Algolia search index storage
resource "aws_s3_bucket" "algolia_index_bucket" {
  bucket = "qanto-docs-search-index-${var.environment}-${random_id.bucket_suffix.hex}"
}

# S3 bucket for logs
resource "aws_s3_bucket" "logs_bucket" {
  bucket = "qanto-docs-logs-${var.environment}-${random_id.bucket_suffix.hex}"
}

resource "aws_s3_bucket_server_side_encryption_configuration" "logs_bucket_encryption" {
  bucket = aws_s3_bucket.logs_bucket.id

  rule {
    apply_server_side_encryption_by_default {
      sse_algorithm = "AES256"
    }
  }
}

# CloudWatch Log Group for monitoring
resource "aws_cloudwatch_log_group" "docs_logs" {
  name              = "/aws/cloudfront/qanto-docs"
  retention_in_days = 30
}

# Lambda function for search indexing (Algolia integration)
resource "aws_lambda_function" "search_indexer" {
  filename         = "search_indexer.zip"
  function_name    = "qanto-docs-search-indexer"
  role            = aws_iam_role.lambda_execution_role.arn
  handler         = "index.handler"
  runtime         = "nodejs18.x"
  timeout         = 300

  environment {
    variables = {
      ALGOLIA_APP_ID     = var.algolia_app_id
      ALGOLIA_ADMIN_KEY  = var.algolia_admin_key
      ALGOLIA_INDEX_NAME = "qanto-docs"
    }
  }
}

# IAM role for Lambda function
resource "aws_iam_role" "lambda_execution_role" {
  name = "qanto-docs-lambda-execution-role"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Action = "sts:AssumeRole"
        Effect = "Allow"
        Principal = {
          Service = "lambda.amazonaws.com"
        }
      }
    ]
  })
}

resource "aws_iam_role_policy_attachment" "lambda_basic_execution" {
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
  role       = aws_iam_role.lambda_execution_role.name
}

# Variables for Algolia configuration
variable "algolia_app_id" {
  description = "Algolia Application ID"
  type        = string
  sensitive   = true
}

variable "algolia_admin_key" {
  description = "Algolia Admin API Key"
  type        = string
  sensitive   = true
}

# Output values
output "docs_bucket_name" {
  description = "Name of the S3 bucket hosting the docs"
  value       = aws_s3_bucket.docs_bucket.bucket
}

output "cloudfront_distribution_id" {
  description = "CloudFront distribution ID"
  value       = aws_cloudfront_distribution.docs_distribution.id
}

output "docs_url" {
  description = "URL of the documentation site"
  value       = "https://${var.domain_name}"
}

output "github_actions_role_arn" {
  description = "ARN of the GitHub Actions IAM role"
  value       = aws_iam_role.github_actions_role.arn
}

# Local values for common tags
locals {
  common_tags = {
    Project     = "qanto-docs"
    Environment = var.environment
    ManagedBy   = "terraform"
    Team        = "infrastructure"
  }
}
