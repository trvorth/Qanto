# ============================================================================
# Enhanced Monitoring and Alerting for Qanto Production Infrastructure
# Additional CloudWatch metrics, custom metrics, and alerting rules
# ============================================================================

# ============================================================================
# Custom CloudWatch Metrics for Application Performance
# ============================================================================

# Lambda function for custom metrics collection
resource "aws_lambda_function" "custom_metrics_collector" {
  filename         = "custom-metrics-collector.zip"
  function_name    = "qanto-custom-metrics-collector"
  role            = aws_iam_role.lambda_metrics_role.arn
  handler         = "index.handler"
  runtime         = "nodejs18.x"
  timeout         = 60
  
  environment {
    variables = {
      CLUSTER_NAME = var.cluster_name
      DOMAIN_NAME  = var.domain_name
      AWS_REGION   = var.aws_region
    }
  }
  
  tags = {
    Name = "qanto-custom-metrics-collector"
  }
}

# EventBridge rule to trigger custom metrics collection every 5 minutes
resource "aws_cloudwatch_event_rule" "custom_metrics_schedule" {
  name                = "qanto-custom-metrics-schedule"
  description         = "Trigger custom metrics collection every 5 minutes"
  schedule_expression = "rate(5 minutes)"
}

resource "aws_cloudwatch_event_target" "lambda_target" {
  rule      = aws_cloudwatch_event_rule.custom_metrics_schedule.name
  target_id = "CustomMetricsLambdaTarget"
  arn       = aws_lambda_function.custom_metrics_collector.arn
}

resource "aws_lambda_permission" "allow_eventbridge" {
  statement_id  = "AllowExecutionFromEventBridge"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.custom_metrics_collector.function_name
  principal     = "events.amazonaws.com"
  source_arn    = aws_cloudwatch_event_rule.custom_metrics_schedule.arn
}

# ============================================================================
# Additional CloudWatch Alarms for Comprehensive Monitoring
# ============================================================================

# Database connection alarm (if RDS is used)
resource "aws_cloudwatch_metric_alarm" "database_cpu_high" {
  alarm_name          = "qanto-database-cpu-high"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = "2"
  metric_name         = "CPUUtilization"
  namespace           = "AWS/RDS"
  period              = "300"
  statistic           = "Average"
  threshold           = "80"
  alarm_description   = "This metric monitors database CPU utilization"
  alarm_actions       = [aws_sns_topic.alerts.arn]
  
  dimensions = {
    DBInstanceIdentifier = "qanto-production-db"  # Will be updated if RDS is added
  }
}

# ELB unhealthy hosts alarm
resource "aws_cloudwatch_metric_alarm" "unhealthy_hosts" {
  alarm_name          = "qanto-unhealthy-hosts"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = "2"
  metric_name         = "UnHealthyHostCount"
  namespace           = "AWS/ApplicationELB"
  period              = "300"
  statistic           = "Average"
  threshold           = "0"
  alarm_description   = "This metric monitors unhealthy host count"
  alarm_actions       = [aws_sns_topic.alerts.arn]
  
  dimensions = {
    LoadBalancer = "WILL_BE_UPDATED_BY_DEPLOYMENT"
  }
}

# CloudFront error rate alarm
resource "aws_cloudwatch_metric_alarm" "cloudfront_high_error_rate" {
  alarm_name          = "qanto-cloudfront-high-error-rate"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = "3"
  metric_name         = "4xxErrorRate"
  namespace           = "AWS/CloudFront"
  period              = "300"
  statistic           = "Average"
  threshold           = "10"
  alarm_description   = "This metric monitors CloudFront 4xx error rate"
  alarm_actions       = [aws_sns_topic.alerts.arn]
  
  dimensions = {
    DistributionId = aws_cloudfront_distribution.static_assets_cdn.id
  }
}

# S3 bucket size alarm
resource "aws_cloudwatch_metric_alarm" "s3_bucket_size_high" {
  alarm_name          = "qanto-s3-bucket-size-high"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = "1"
  metric_name         = "BucketSizeBytes"
  namespace           = "AWS/S3"
  period              = "86400"  # Daily check
  statistic           = "Average"
  threshold           = "53687091200"  # 50GB in bytes
  alarm_description   = "This metric monitors S3 bucket size"
  alarm_actions       = [aws_sns_topic.alerts.arn]
  
  dimensions = {
    BucketName  = aws_s3_bucket.static_assets.bucket
    StorageType = "StandardStorage"
  }
}

# EKS cluster node group scaling alarm
resource "aws_cloudwatch_metric_alarm" "eks_node_cpu_high" {
  alarm_name          = "qanto-eks-node-cpu-high"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = "3"
  metric_name         = "CPUUtilization"
  namespace           = "AWS/EC2"
  period              = "300"
  statistic           = "Average"
  threshold           = "85"
  alarm_description   = "This metric monitors EKS node CPU utilization"
  alarm_actions       = [aws_sns_topic.alerts.arn]
  
  dimensions = {
    AutoScalingGroupName = "WILL_BE_UPDATED_BY_DEPLOYMENT"
  }
}

# ============================================================================
# SNS Topics and Subscriptions for Alert Management
# ============================================================================

# Critical alerts topic
resource "aws_sns_topic" "critical_alerts" {
  name = "qanto-critical-alerts"
  
  tags = {
    Name = "qanto-critical-alerts"
  }
}

# Warning alerts topic
resource "aws_sns_topic" "warning_alerts" {
  name = "qanto-warning-alerts"
  
  tags = {
    Name = "qanto-warning-alerts"
  }
}

# Email subscription for critical alerts (replace with actual email)
resource "aws_sns_topic_subscription" "critical_email" {
  topic_arn = aws_sns_topic.critical_alerts.arn
  protocol  = "email"
  endpoint  = "alerts@qanto.org"  # Replace with actual email
}

# ============================================================================
# CloudWatch Composite Alarms for Complex Alerting Logic
# ============================================================================

# Application health composite alarm
resource "aws_cloudwatch_composite_alarm" "application_health" {
  alarm_name        = "qanto-application-health"
  alarm_description = "Composite alarm for overall application health"
  
  alarm_rule = join(" OR ", [
    "ALARM(${aws_cloudwatch_metric_alarm.high_error_rate.alarm_name})",
    "ALARM(${aws_cloudwatch_metric_alarm.high_response_time.alarm_name})",
    "ALARM(${aws_cloudwatch_metric_alarm.unhealthy_hosts.alarm_name})"
  ])
  
  alarm_actions = [aws_sns_topic.critical_alerts.arn]
  ok_actions    = [aws_sns_topic.warning_alerts.arn]
  
  tags = {
    Name = "qanto-application-health-composite"
  }
}

# Infrastructure health composite alarm
resource "aws_cloudwatch_composite_alarm" "infrastructure_health" {
  alarm_name        = "qanto-infrastructure-health"
  alarm_description = "Composite alarm for overall infrastructure health"
  
  alarm_rule = join(" OR ", [
    "ALARM(${aws_cloudwatch_metric_alarm.eks_node_cpu_high.alarm_name})",
    "ALARM(${aws_cloudwatch_metric_alarm.cloudfront_high_error_rate.alarm_name})"
  ])
  
  alarm_actions = [aws_sns_topic.warning_alerts.arn]
  
  tags = {
    Name = "qanto-infrastructure-health-composite"
  }
}

# ============================================================================
# Custom CloudWatch Dashboard for Executive Summary
# ============================================================================

resource "aws_cloudwatch_dashboard" "executive_summary" {
  dashboard_name = "Qanto-Executive-Summary"
  
  dashboard_body = jsonencode({
    widgets = [
      {
        type   = "metric"
        x      = 0
        y      = 0
        width  = 24
        height = 6
        
        properties = {
          metrics = [
            ["AWS/ApplicationELB", "RequestCount", "LoadBalancer", "WILL_BE_UPDATED"],
            ["AWS/CloudFront", "Requests", "DistributionId", aws_cloudfront_distribution.static_assets_cdn.id]
          ]
          view    = "timeSeries"
          stacked = false
          region  = var.aws_region
          title   = "Total Requests - Last 24 Hours"
          period  = 3600
          stat    = "Sum"
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
            ["AWS/ApplicationELB", "TargetResponseTime", "LoadBalancer", "WILL_BE_UPDATED"],
            ["AWS/CloudFront", "OriginLatency", "DistributionId", aws_cloudfront_distribution.static_assets_cdn.id]
          ]
          view    = "timeSeries"
          stacked = false
          region  = var.aws_region
          title   = "Response Time Performance"
          period  = 300
          stat    = "Average"
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
            ["AWS/ApplicationELB", "HTTPCode_Target_2XX_Count", "LoadBalancer", "WILL_BE_UPDATED"],
            [".", "HTTPCode_Target_4XX_Count", ".", "."],
            [".", "HTTPCode_Target_5XX_Count", ".", "."]
          ]
          view    = "timeSeries"
          stacked = false
          region  = var.aws_region
          title   = "HTTP Response Codes"
          period  = 300
          stat    = "Sum"
        }
      },
      {
        type   = "number"
        x      = 0
        y      = 12
        width  = 6
        height = 6
        
        properties = {
          metrics = [
            ["AWS/ApplicationELB", "ActiveConnectionCount", "LoadBalancer", "WILL_BE_UPDATED"]
          ]
          region = var.aws_region
          title  = "Active Connections"
          stat   = "Average"
          period = 300
        }
      },
      {
        type   = "number"
        x      = 6
        y      = 12
        width  = 6
        height = 6
        
        properties = {
          metrics = [
            ["AWS/S3", "BucketSizeBytes", "BucketName", aws_s3_bucket.static_assets.bucket, "StorageType", "StandardStorage"]
          ]
          region = var.aws_region
          title  = "Static Assets Storage (GB)"
          stat   = "Average"
          period = 86400
        }
      },
      {
        type   = "number"
        x      = 12
        y      = 12
        width  = 6
        height = 6
        
        properties = {
          metrics = [
            ["AWS/CloudFront", "Requests", "DistributionId", aws_cloudfront_distribution.static_assets_cdn.id]
          ]
          region = "us-east-1"
          title  = "CDN Requests (24h)"
          stat   = "Sum"
          period = 86400
        }
      },
      {
        type   = "number"
        x      = 18
        y      = 12
        width  = 6
        height = 6
        
        properties = {
          metrics = [
            ["AWS/EKS", "cluster_number_of_running_pods", "ClusterName", var.cluster_name]
          ]
          region = var.aws_region
          title  = "Running Pods"
          stat   = "Average"
          period = 300
        }
      }
    ]
  })
}

# ============================================================================
# IAM Role for Lambda Metrics Collection
# ============================================================================

resource "aws_iam_role" "lambda_metrics_role" {
  name = "${var.cluster_name}-lambda-metrics-role"
  
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
  role       = aws_iam_role.lambda_metrics_role.name
}

resource "aws_iam_role_policy" "lambda_metrics_policy" {
  name = "${var.cluster_name}-lambda-metrics-policy"
  role = aws_iam_role.lambda_metrics_role.id
  
  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Action = [
          "cloudwatch:PutMetricData",
          "cloudwatch:GetMetricStatistics",
          "cloudwatch:ListMetrics"
        ]
        Resource = "*"
      },
      {
        Effect = "Allow"
        Action = [
          "eks:DescribeCluster",
          "eks:ListNodegroups",
          "eks:DescribeNodegroup"
        ]
        Resource = module.eks.cluster_arn
      },
      {
        Effect = "Allow"
        Action = [
          "elbv2:DescribeLoadBalancers",
          "elbv2:DescribeTargetHealth"
        ]
        Resource = "*"
      }
    ]
  })
}

# ============================================================================
# Outputs for Monitoring Integration
# ============================================================================

output "custom_metrics_lambda_arn" {
  description = "ARN of the custom metrics Lambda function"
  value       = aws_lambda_function.custom_metrics_collector.arn
}

output "critical_alerts_topic_arn" {
  description = "ARN of the critical alerts SNS topic"
  value       = aws_sns_topic.critical_alerts.arn
}

output "warning_alerts_topic_arn" {
  description = "ARN of the warning alerts SNS topic"
  value       = aws_sns_topic.warning_alerts.arn
}

output "executive_dashboard_url" {
  description = "URL to the executive summary CloudWatch dashboard"
  value       = "https://console.aws.amazon.com/cloudwatch/home?region=${var.aws_region}#dashboards:name=Qanto-Executive-Summary"
}
