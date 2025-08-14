# ============================================================================
# ACM Certificate with Route53 DNS Validation for Qanto Domain
# ============================================================================

locals {
  # If a hosted zone ID is provided, use it; otherwise create a new public hosted zone
  use_existing_zone = var.route53_zone_id != null && var.route53_zone_id != ""
}

# Optionally create a new public hosted zone if none provided
resource "aws_route53_zone" "qanto" {
  count = local.use_existing_zone ? 0 : 1

  name = var.domain_name
  comment = "Hosted zone for ${var.domain_name} (managed by Terraform)"
}

# Resolve hosted zone ID (existing or created)
locals {
  hosted_zone_id = local.use_existing_zone ? var.route53_zone_id : aws_route53_zone.qanto[0].zone_id
}

# Request ACM certificate in the same region as the ALB (must be regional, not us-east-1 for CloudFront)
resource "aws_acm_certificate" "qanto" {
  domain_name               = var.domain_name
  validation_method         = "DNS"
  subject_alternative_names = ["www.${var.domain_name}"]

  lifecycle {
    create_before_destroy = true
  }

  tags = {
    Name        = "qanto-acm-${var.domain_name}"
    Environment = var.environment
  }
}

# Create DNS validation records in Route53 using the hosted zone
resource "aws_route53_record" "qanto_cert_validation" {
  for_each = {
    for dvo in aws_acm_certificate.qanto.domain_validation_options : dvo.domain_name => {
      name   = dvo.resource_record_name
      record = dvo.resource_record_value
      type   = dvo.resource_record_type
    }
  }

  zone_id = local.hosted_zone_id
  name    = each.value.name
  type    = each.value.type
  ttl     = 60
  records = [each.value.record]
}

# Complete ACM certificate validation
resource "aws_acm_certificate_validation" "qanto" {
  certificate_arn         = aws_acm_certificate.qanto.arn
  validation_record_fqdns = [for record in aws_route53_record.qanto_cert_validation : record.fqdn]
}

# Outputs
output "acm_certificate_arn" {
  description = "ARN of the validated ACM certificate for the domain"
  value       = aws_acm_certificate.qanto.arn
}

output "hosted_zone_id" {
  description = "Hosted Zone ID used for DNS validation (existing or created)"
  value       = local.hosted_zone_id
}
