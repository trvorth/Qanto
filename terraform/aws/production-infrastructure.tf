# ============================================================================
# Qanto Production Infrastructure - Comprehensive AWS Setup
# CloudFront CDN, Route53 DNS, ACM Certificates, S3 Static Assets, Monitoring
# ============================================================================

# Additional provider for us-east-1 (required for CloudFront certificates)
provider "aws" {
  alias  = "us_east_1"
  region = "us-east-1"
  
  default_tags {
    tags = {
      Project     = "Qanto"
      Environment = var.environment
      ManagedBy   = "Terraform"
    }
  }
}

# ============================================================================
# Route53 DNS Configuration for qanto.org and subdomains
# ============================================================================

# Main qanto.org hosted zone (managed by Terraform)
data "aws_route53_zone" "qanto_org" {
  zone_id = "Z01616693C5C3RA5L6IQR"
}

# ============================================================================
# ACM Certificates for all subdomains with auto-renewal
# ============================================================================

# Wildcard certificate for CloudFront (must be in us-east-1)
resource "aws_acm_certificate" "qanto_wildcard_cloudfront" {
  provider = aws.us_east_1
  
  domain_name               = "qanto.org"
  subject_alternative_names = ["*.qanto.org"]
  validation_method         = "DNS"
  
  lifecycle {
    create_before_destroy = true
  }
  
  tags = {
    Name = "qanto-org-wildcard-cloudfront"
  }
}

# Regional certificate for ALB (in the primary region)
resource "aws_acm_certificate" "qanto_wildcard_regional" {
  domain_name               = "qanto.org"
  subject_alternative_names = ["*.qanto.org"]
  validation_method         = "DNS"
  
  lifecycle {
    create_before_destroy = true
  }
  
  tags = {
    Name = "qanto-org-wildcard-regional"
  }
}

# DNS validation records for CloudFront certificate
resource "aws_route53_record" "qanto_cert_validation_cloudfront" {
  for_each = {
    for dvo in aws_acm_certificate.qanto_wildcard_cloudfront.domain_validation_options : dvo.domain_name => {
      name   = dvo.resource_record_name
      record = dvo.resource_record_value
      type   = dvo.resource_record_type
    }
  }
  
  zone_id = data.aws_route53_zone.qanto_org.zone_id
  name    = each.value.name
  type    = each.value.type
  ttl     = 60
  records = [each.value.record]
}

# DNS validation records for regional certificate
resource "aws_route53_record" "qanto_cert_validation_regional" {
  for_each = {
    for dvo in aws_acm_certificate.qanto_wildcard_regional.domain_validation_options : dvo.domain_name => {
      name   = dvo.resource_record_name
      record = dvo.resource_record_value
      type   = dvo.resource_record_type
    }
  }
  
  zone_id = data.aws_route53_zone.qanto_org.zone_id
  name    = each.value.name
  type    = each.value.type
  ttl     = 60
  records = [each.value.record]
}

# Certificate validation
resource "aws_acm_certificate_validation" "qanto_wildcard_cloudfront" {
  provider = aws.us_east_1
  
  certificate_arn         = aws_acm_certificate.qanto_wildcard_cloudfront.arn
  validation_record_fqdns = [for record in aws_route53_record.qanto_cert_validation_cloudfront : record.fqdn]
}

resource "aws_acm_certificate_validation" "qanto_wildcard_regional" {
  certificate_arn         = aws_acm_certificate.qanto_wildcard_regional.arn
  validation_record_fqdns = [for record in aws_route53_record.qanto_cert_validation_regional : record.fqdn]
}

# ============================================================================
# S3 Buckets for Static Assets and Website
# ============================================================================

# Main static assets bucket
resource "aws_s3_bucket" "static_assets" {
  bucket = "qanto-static-assets-${var.environment}-${random_id.bucket_suffix.hex}"
}

resource "random_id" "bucket_suffix" {
  byte_length = 8
}

# Website build artifacts bucket
resource "aws_s3_bucket" "website_builds" {
  bucket = "qanto-website-builds-${var.environment}-${random_id.bucket_suffix.hex}"
}

# CloudWatch logs bucket
resource "aws_s3_bucket" "cloudwatch_logs" {
  bucket = "qanto-cloudwatch-logs-${var.environment}-${random_id.bucket_suffix.hex}"
}

# Configure static assets bucket
resource "aws_s3_bucket_versioning" "static_assets_versioning" {
  bucket = aws_s3_bucket.static_assets.id
  versioning_configuration {
    status = "Enabled"
  }
}

resource "aws_s3_bucket_server_side_encryption_configuration" "static_assets_encryption" {
  bucket = aws_s3_bucket.static_assets.id
  
  rule {
    apply_server_side_encryption_by_default {
      sse_algorithm = "AES256"
    }
  }
}

resource "aws_s3_bucket_public_access_block" "static_assets_pab" {
  bucket = aws_s3_bucket.static_assets.id
  
  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}

# Configure website builds bucket
resource "aws_s3_bucket_versioning" "website_builds_versioning" {
  bucket = aws_s3_bucket.website_builds.id
  versioning_configuration {
    status = "Enabled"
  }
}

resource "aws_s3_bucket_server_side_encryption_configuration" "website_builds_encryption" {
  bucket = aws_s3_bucket.website_builds.id
  
  rule {
    apply_server_side_encryption_by_default {
      sse_algorithm = "AES256"
    }
  }
}

# S3 bucket lifecycle policies
resource "aws_s3_bucket_lifecycle_configuration" "static_assets_lifecycle" {
  bucket = aws_s3_bucket.static_assets.id
  
  rule {
    id     = "delete_old_versions"
    status = "Enabled"
    
    filter {
      prefix = ""
    }
    
    noncurrent_version_expiration {
      noncurrent_days = 30
    }
  }
}

# ============================================================================
# CloudFront CDN Distribution for Global Content Delivery
# ============================================================================

# Origin Access Control for S3
resource "aws_cloudfront_origin_access_control" "static_assets_oac" {
  name                              = "qanto-static-assets-oac"
  description                       = "Origin Access Control for Qanto Static Assets"
  origin_access_control_origin_type = "s3"
  signing_behavior                  = "always"
  signing_protocol                  = "sigv4"
}

# CloudFront distribution for static assets
resource "aws_cloudfront_distribution" "static_assets_cdn" {
  enabled             = true
  is_ipv6_enabled     = true
  comment             = "Qanto Static Assets CDN"
  default_root_object = "index.html"
  
  # Multiple origins for different content types
  origin {
    domain_name              = aws_s3_bucket.static_assets.bucket_regional_domain_name
    origin_access_control_id = aws_cloudfront_origin_access_control.static_assets_oac.id
    origin_id                = "S3-static-assets"
    
    custom_origin_config {
      http_port              = 80
      https_port             = 443
      origin_protocol_policy = "https-only"
      origin_ssl_protocols   = ["TLSv1.2"]
    }
  }
  
  # API origin (ALB)
  origin {
    domain_name = "api.qanto.org"  # This will be set by ALB
    origin_id   = "ALB-api"
    
    custom_origin_config {
      http_port              = 80
      https_port             = 443
      origin_protocol_policy = "https-only"
      origin_ssl_protocols   = ["TLSv1.2"]
    }
  }
  
  aliases = ["cdn.qanto.org", "assets.qanto.org"]
  
  # Default cache behavior for static assets
  default_cache_behavior {
    allowed_methods        = ["DELETE", "GET", "HEAD", "OPTIONS", "PATCH", "POST", "PUT"]
    cached_methods         = ["GET", "HEAD"]
    target_origin_id       = "S3-static-assets"
    compress               = true
    viewer_protocol_policy = "redirect-to-https"
    
    forwarded_values {
      query_string = false
      headers      = ["Origin", "Access-Control-Request-Headers", "Access-Control-Request-Method"]
      
      cookies {
        forward = "none"
      }
    }
    
    min_ttl     = 0
    default_ttl = 86400    # 1 day
    max_ttl     = 31536000 # 1 year
  }
  
  # Cache behavior for images and videos
  ordered_cache_behavior {
    path_pattern           = "/assets/images/*"
    allowed_methods        = ["GET", "HEAD"]
    cached_methods         = ["GET", "HEAD"]
    target_origin_id       = "S3-static-assets"
    compress              = true
    viewer_protocol_policy = "redirect-to-https"
    
    forwarded_values {
      query_string = false
      cookies {
        forward = "none"
      }
    }
    
    min_ttl     = 86400     # 1 day
    default_ttl = 604800    # 1 week
    max_ttl     = 31536000  # 1 year
  }
  
  # Cache behavior for videos
  ordered_cache_behavior {
    path_pattern           = "/assets/videos/*"
    allowed_methods        = ["GET", "HEAD"]
    cached_methods         = ["GET", "HEAD"]
    target_origin_id       = "S3-static-assets"
    compress              = false  # Videos are already compressed
    viewer_protocol_policy = "redirect-to-https"
    
    forwarded_values {
      query_string = false
      cookies {
        forward = "none"
      }
    }
    
    min_ttl     = 86400     # 1 day
    default_ttl = 2592000   # 30 days
    max_ttl     = 31536000  # 1 year
  }
  
  # Cache behavior for downloadable resources
  ordered_cache_behavior {
    path_pattern           = "/downloads/*"
    allowed_methods        = ["GET", "HEAD"]
    cached_methods         = ["GET", "HEAD"]
    target_origin_id       = "S3-static-assets"
    compress              = true
    viewer_protocol_policy = "redirect-to-https"
    
    forwarded_values {
      query_string = true
      cookies {
        forward = "none"
      }
    }
    
    min_ttl     = 3600      # 1 hour
    default_ttl = 86400     # 1 day
    max_ttl     = 604800    # 1 week
  }
  
  # API cache behavior
  ordered_cache_behavior {
    path_pattern           = "/api/*"
    allowed_methods        = ["DELETE", "GET", "HEAD", "OPTIONS", "PATCH", "POST", "PUT"]
    cached_methods         = ["GET", "HEAD"]
    target_origin_id       = "ALB-api"
    compress              = true
    viewer_protocol_policy = "redirect-to-https"
    
    forwarded_values {
      query_string = true
      headers      = ["Authorization", "Content-Type", "X-Requested-With"]
      
      cookies {
        forward = "none"
      }
    }
    
    min_ttl     = 0
    default_ttl = 300      # 5 minutes
    max_ttl     = 3600     # 1 hour
  }
  
  price_class = "PriceClass_All"  # Global distribution
  
  restrictions {
    geo_restriction {
      restriction_type = "none"
    }
  }
  
  viewer_certificate {
    acm_certificate_arn      = aws_acm_certificate_validation.qanto_wildcard_cloudfront.certificate_arn
    ssl_support_method       = "sni-only"
    minimum_protocol_version = "TLSv1.2_2021"
  }
  
  # Custom error responses
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
  
  # Enable logging
  logging_config {
    bucket          = aws_s3_bucket.cloudwatch_logs.bucket_domain_name
    prefix          = "cloudfront-access-logs/"
    include_cookies = false
  }
  
  tags = {
    Name = "qanto-static-assets-cdn"
  }
}

# S3 bucket policy for CloudFront access
resource "aws_s3_bucket_policy" "static_assets_cloudfront_policy" {
  bucket = aws_s3_bucket.static_assets.id
  
  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Sid    = "AllowCloudFrontServicePrincipal"
        Effect = "Allow"
        Principal = {
          Service = "cloudfront.amazonaws.com"
        }
        Action   = "s3:GetObject"
        Resource = "${aws_s3_bucket.static_assets.arn}/*"
        Condition = {
          StringEquals = {
            "AWS:SourceArn" = aws_cloudfront_distribution.static_assets_cdn.arn
          }
        }
      }
    ]
  })
}

# ============================================================================
# Route53 DNS Records for all subdomains
# ============================================================================

# Main website (qanto.org)
resource "aws_route53_record" "website" {
  zone_id = data.aws_route53_zone.qanto_org.zone_id
  name    = "qanto.org"
  type    = "A"
  
  alias {
    name                   = "WILL_BE_UPDATED_BY_HELM"  # ALB DNS name
    zone_id                = "WILL_BE_UPDATED_BY_HELM"  # ALB zone ID
    evaluate_target_health = true
  }
}

# www.qanto.org (redirect to main)
resource "aws_route53_record" "website_www" {
  zone_id = data.aws_route53_zone.qanto_org.zone_id
  name    = "www.qanto.org"
  type    = "A"
  
  alias {
    name                   = "WILL_BE_UPDATED_BY_HELM"  # ALB DNS name
    zone_id                = "WILL_BE_UPDATED_BY_HELM"  # ALB zone ID
    evaluate_target_health = true
  }
}

# docs.qanto.org (documentation portal)
resource "aws_route53_record" "docs" {
  zone_id = data.aws_route53_zone.qanto_org.zone_id
  name    = "docs.qanto.org"
  type    = "A"
  
  alias {
    name                   = "WILL_BE_UPDATED_BY_HELM"  # ALB DNS name
    zone_id                = "WILL_BE_UPDATED_BY_HELM"  # ALB zone ID
    evaluate_target_health = true
  }
}

# api.qanto.org (API gateway)
resource "aws_route53_record" "api" {
  zone_id = data.aws_route53_zone.qanto_org.zone_id
  name    = "api.qanto.org"
  type    = "A"
  
  alias {
    name                   = "WILL_BE_UPDATED_BY_HELM"  # ALB DNS name
    zone_id                = "WILL_BE_UPDATED_BY_HELM"  # ALB zone ID
    evaluate_target_health = true
  }
}

# testnet.qanto.org (testnet explorer/tools)
resource "aws_route53_record" "testnet" {
  zone_id = data.aws_route53_zone.qanto_org.zone_id
  name    = "testnet.qanto.org"
  type    = "A"
  
  alias {
    name                   = "WILL_BE_UPDATED_BY_HELM"  # ALB DNS name
    zone_id                = "WILL_BE_UPDATED_BY_HELM"  # ALB zone ID
    evaluate_target_health = true
  }
}

# explorer.qanto.org (blockchain explorer)
resource "aws_route53_record" "explorer" {
  zone_id = data.aws_route53_zone.qanto_org.zone_id
  name    = "explorer.qanto.org"
  type    = "A"
  
  alias {
    name                   = "WILL_BE_UPDATED_BY_HELM"  # ALB DNS name
    zone_id                = "WILL_BE_UPDATED_BY_HELM"  # ALB zone ID
    evaluate_target_health = true
  }
}

# cdn.qanto.org (CDN alias for static assets)
resource "aws_route53_record" "cdn" {
  zone_id = data.aws_route53_zone.qanto_org.zone_id
  name    = "cdn.qanto.org"
  type    = "A"
  
  alias {
    name                   = aws_cloudfront_distribution.static_assets_cdn.domain_name
    zone_id                = aws_cloudfront_distribution.static_assets_cdn.hosted_zone_id
    evaluate_target_health = false
  }
}

# assets.qanto.org (assets alias)
resource "aws_route53_record" "assets" {
  zone_id = data.aws_route53_zone.qanto_org.zone_id
  name    = "assets.qanto.org"
  type    = "A"
  
  alias {
    name                   = aws_cloudfront_distribution.static_assets_cdn.domain_name
    zone_id                = aws_cloudfront_distribution.static_assets_cdn.hosted_zone_id
    evaluate_target_health = false
  }
}

# IPv6 records for CloudFront
resource "aws_route53_record" "cdn_ipv6" {
  zone_id = data.aws_route53_zone.qanto_org.zone_id
  name    = "cdn.qanto.org"
  type    = "AAAA"
  
  alias {
    name                   = aws_cloudfront_distribution.static_assets_cdn.domain_name
    zone_id                = aws_cloudfront_distribution.static_assets_cdn.hosted_zone_id
    evaluate_target_health = false
  }
}

# ============================================================================
# CloudWatch Dashboards and Monitoring
# ============================================================================

# CloudWatch dashboard for application metrics
resource "aws_cloudwatch_dashboard" "qanto_app_dashboard" {
  dashboard_name = "Qanto-Application-Metrics"
  
  dashboard_body = jsonencode({
    widgets = [
      {
        type   = "metric"
        x      = 0
        y      = 0
        width  = 12
        height = 6
        
        properties = {
          metrics = [
            ["AWS/ApplicationELB", "RequestCount", "LoadBalancer", "WILL_BE_UPDATED"],
            [".", "TargetResponseTime", ".", "."],
            [".", "HTTPCode_Target_2XX_Count", ".", "."],
            [".", "HTTPCode_Target_4XX_Count", ".", "."],
            [".", "HTTPCode_Target_5XX_Count", ".", "."]
          ]
          view    = "timeSeries"
          stacked = false
          region  = var.aws_region
          title   = "Application Load Balancer Metrics"
          period  = 300
        }
      },
      {
        type   = "metric"
        x      = 12
        y      = 0
        width  = 12
        height = 6
        
        properties = {
          metrics = [
            ["AWS/EKS", "cluster_failed_request_count", "ClusterName", var.cluster_name],
            [".", "cluster_node_count", ".", "."],
            [".", "cluster_number_of_running_pods", ".", "."]
          ]
          view    = "timeSeries"
          stacked = false
          region  = var.aws_region
          title   = "EKS Cluster Metrics"
          period  = 300
        }
      },
      {
        type   = "metric"
        x      = 0
        y      = 6
        width  = 12
        height = 6
        
        properties = {
          metrics = [
            ["AWS/CloudFront", "Requests", "DistributionId", aws_cloudfront_distribution.static_assets_cdn.id],
            [".", "BytesDownloaded", ".", "."],
            [".", "BytesUploaded", ".", "."],
            [".", "4xxErrorRate", ".", "."],
            [".", "5xxErrorRate", ".", "."]
          ]
          view    = "timeSeries"
          stacked = false
          region  = "us-east-1"  # CloudFront metrics are always in us-east-1
          title   = "CloudFront CDN Metrics"
          period  = 300
        }
      },
      {
        type   = "metric"
        x      = 12
        y      = 6
        width  = 12
        height = 6
        
        properties = {
          metrics = [
            ["AWS/S3", "BucketSizeBytes", "BucketName", aws_s3_bucket.static_assets.bucket, "StorageType", "StandardStorage"],
            [".", "NumberOfObjects", ".", ".", ".", "AllStorageTypes"]
          ]
          view    = "timeSeries"
          stacked = false
          region  = var.aws_region
          title   = "S3 Storage Metrics"
          period  = 86400  # Daily
        }
      }
    ]
  })
}

# CloudWatch dashboard for infrastructure metrics
resource "aws_cloudwatch_dashboard" "qanto_infra_dashboard" {
  dashboard_name = "Qanto-Infrastructure-Metrics"
  
  dashboard_body = jsonencode({
    widgets = [
      {
        type   = "metric"
        x      = 0
        y      = 0
        width  = 12
        height = 6
        
        properties = {
          metrics = [
            ["AWS/EC2", "CPUUtilization", "AutoScalingGroupName", "WILL_BE_UPDATED"],
            [".", "NetworkIn", ".", "."],
            [".", "NetworkOut", ".", "."]
          ]
          view    = "timeSeries"
          stacked = false
          region  = var.aws_region
          title   = "EC2 Instance Metrics"
          period  = 300
        }
      },
      {
        type   = "metric"
        x      = 12
        y      = 0
        width  = 12
        height = 6
        
        properties = {
          metrics = [
            ["AWS/Route53", "QueryCount", "HostedZoneId", data.aws_route53_zone.qanto_org.zone_id],
            ["AWS/Route53Resolver", "InboundQueryCount", "ResolverEndpointId", "WILL_BE_UPDATED"],
            [".", "OutboundQueryCount", ".", "."]
          ]
          view    = "timeSeries"
          stacked = false
          region  = var.aws_region
          title   = "DNS Query Metrics"
          period  = 300
        }
      }
    ]
  })
}

# CloudWatch Log Groups
resource "aws_cloudwatch_log_group" "eks_cluster_logs" {
  name              = "/aws/eks/${var.cluster_name}/cluster"
  retention_in_days = 30
}

resource "aws_cloudwatch_log_group" "application_logs" {
  name              = "/aws/eks/${var.cluster_name}/application"
  retention_in_days = 7
}

resource "aws_cloudwatch_log_group" "cloudfront_logs" {
  name              = "/aws/cloudfront/qanto-static-assets"
  retention_in_days = 14
}

# ============================================================================
# CloudWatch Alarms for Critical Metrics
# ============================================================================

# High error rate alarm
resource "aws_cloudwatch_metric_alarm" "high_error_rate" {
  alarm_name          = "qanto-high-error-rate"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = "2"
  metric_name         = "HTTPCode_Target_5XX_Count"
  namespace           = "AWS/ApplicationELB"
  period              = "300"
  statistic           = "Sum"
  threshold           = "10"
  alarm_description   = "This metric monitors 5xx error count"
  alarm_actions       = [aws_sns_topic.alerts.arn]
  
  dimensions = {
    LoadBalancer = "WILL_BE_UPDATED_BY_DEPLOYMENT"
  }
}

# High response time alarm
resource "aws_cloudwatch_metric_alarm" "high_response_time" {
  alarm_name          = "qanto-high-response-time"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = "2"
  metric_name         = "TargetResponseTime"
  namespace           = "AWS/ApplicationELB"
  period              = "300"
  statistic           = "Average"
  threshold           = "2"  # 2 seconds
  alarm_description   = "This metric monitors response time"
  alarm_actions       = [aws_sns_topic.alerts.arn]
  
  dimensions = {
    LoadBalancer = "WILL_BE_UPDATED_BY_DEPLOYMENT"
  }
}

# SNS topic for alerts
resource "aws_sns_topic" "alerts" {
  name = "qanto-infrastructure-alerts"
}

# ============================================================================
# IAM Roles for Enhanced Services
# ============================================================================

# Role for CloudWatch Container Insights
module "container_insights_irsa" {
  source  = "terraform-aws-modules/iam/aws//modules/iam-role-for-service-accounts-eks"
  version = "~> 5.0"
  
  role_name = "${var.cluster_name}-container-insights"
  
  role_policy_arns = {
    CloudWatchAgentServerPolicy = "arn:aws:iam::aws:policy/CloudWatchAgentServerPolicy"
  }
  
  oidc_providers = {
    main = {
      provider_arn               = module.eks.oidc_provider_arn
      namespace_service_accounts = ["amazon-cloudwatch:cloudwatch-agent"]
    }
  }
}

# Role for S3 access from applications
module "s3_access_irsa" {
  source  = "terraform-aws-modules/iam/aws//modules/iam-role-for-service-accounts-eks"
  version = "~> 5.0"
  
  role_name = "${var.cluster_name}-s3-access"
  
  role_policy_arns = {
    S3Access = aws_iam_policy.s3_assets_access.arn
  }
  
  oidc_providers = {
    main = {
      provider_arn               = module.eks.oidc_provider_arn
      namespace_service_accounts = ["default:qanto-website"]
    }
  }
}

# Custom IAM policy for S3 assets access
resource "aws_iam_policy" "s3_assets_access" {
  name        = "${var.cluster_name}-s3-assets-access"
  description = "Policy for accessing Qanto static assets S3 bucket"
  
  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Action = [
          "s3:GetObject",
          "s3:GetObjectVersion",
          "s3:PutObject",
          "s3:PutObjectAcl",
          "s3:DeleteObject",
          "s3:ListBucket"
        ]
        Resource = [
          aws_s3_bucket.static_assets.arn,
          "${aws_s3_bucket.static_assets.arn}/*"
        ]
      }
    ]
  })
}

# ============================================================================
# Outputs for Integration
# ============================================================================

output "cloudfront_distribution_id" {
  description = "CloudFront distribution ID for cache invalidation"
  value       = aws_cloudfront_distribution.static_assets_cdn.id
}

output "cloudfront_distribution_domain" {
  description = "CloudFront distribution domain name"
  value       = aws_cloudfront_distribution.static_assets_cdn.domain_name
}

output "static_assets_bucket" {
  description = "S3 bucket name for static assets"
  value       = aws_s3_bucket.static_assets.bucket
}

output "website_builds_bucket" {
  description = "S3 bucket name for website builds"
  value       = aws_s3_bucket.website_builds.bucket
}

output "acm_certificate_arn_regional" {
  description = "ACM certificate ARN for ALB (regional)"
  value       = aws_acm_certificate_validation.qanto_wildcard_regional.certificate_arn
}

output "acm_certificate_arn_cloudfront" {
  description = "ACM certificate ARN for CloudFront (us-east-1)"
  value       = aws_acm_certificate_validation.qanto_wildcard_cloudfront.certificate_arn
}

output "route53_zone_id" {
  description = "Route53 hosted zone ID for qanto.org"
  value       = data.aws_route53_zone.qanto_org.zone_id
}

output "container_insights_role_arn" {
  description = "IAM role ARN for Container Insights"
  value       = module.container_insights_irsa.iam_role_arn
}

output "s3_access_role_arn" {
  description = "IAM role ARN for S3 assets access"
  value       = module.s3_access_irsa.iam_role_arn
}
