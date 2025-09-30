# Qanto Deployment Strategy - Namecheap Integration

This document outlines comprehensive deployment strategies for both the Qanto Block Explorer and the main Qanto website using Namecheap's hosting services and third-party integrations.

## Table of Contents

1. [Overview](#overview)
2. [Qanto Block Explorer Deployment](#qanto-block-explorer-deployment)
3. [Qanto Main Website Deployment](#qanto-main-website-deployment)
4. [Domain Management](#domain-management)
5. [SSL/TLS Configuration](#ssltls-configuration)
6. [CDN Integration](#cdn-integration)
7. [Monitoring and Analytics](#monitoring-and-analytics)
8. [Backup and Recovery](#backup-and-recovery)
9. [Security Considerations](#security-considerations)
10. [Maintenance and Updates](#maintenance-and-updates)

## Overview

### Deployment Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     DNS                            │
│  ┌─────────────────┐    ┌─────────────────────────────────┐ │
│  │ explorer.qanto  │    │      www.qanto.org             │ │
│  │     .org        │    │                                 │ │
│  └─────────────────┘    └─────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
           │                              │
           ▼                              ▼
┌─────────────────┐              ┌─────────────────┐
│   Cloudflare    │              │   Cloudflare    │
│      CDN        │              │      CDN        │
└─────────────────┘              └─────────────────┘
           │                              │
           ▼                              ▼
┌─────────────────┐              ┌─────────────────┐
│                 │              │                 │
│   Hosting       │              │    Hosting      │
│  (Explorer)     │              │  (Main Site)    │
└─────────────────┘              └─────────────────┘
```

### Key Requirements

- **High Availability**: 99.9% uptime guarantee
- **Global Performance**: CDN-accelerated content delivery
- **Security**: SSL/TLS encryption, DDoS protection
- **Scalability**: Auto-scaling capabilities for traffic spikes
- **Monitoring**: Real-time performance and uptime monitoring

## Qanto Block Explorer Deployment

### 1. Hosting Solution: Namecheap Shared Hosting

#### Service Selection
- **Plan**: Stellar Plus Shared Hosting
- **Features**:
  - Unlimited websites
  - Unlimited SSD storage
  - Unmetered bandwidth
  - Free SSL certificate
  - cPanel control panel
  - 24/7 support

#### Technical Specifications
- **Storage**: SSD-based for fast loading
- **Bandwidth**: Unmetered for high traffic
- **PHP Version**: 8.1+ (for potential backend features)
- **Database**: MySQL 8.0 (if needed for analytics)

### 2. Domain Configuration

#### Subdomain Setup
```
Subdomain: explorer.qanto.org
Type: A Record
Value: [Namecheap hosting IP]
TTL: 300 seconds
```

#### DNS Records
```
# Primary A Record
explorer.qanto.org.    300    IN    A    [Hosting IP]

# CNAME for www
www.explorer.qanto.org. 300   IN    CNAME explorer.qanto.org.

# MX Records (if email needed)
explorer.qanto.org.    300    IN    MX    10 mail.qanto.org.
```

### 3. File Structure and Deployment

#### Directory Structure
```
deployment/public_html/explorer/
├── index.html (qanto-explorer.html renamed)
├── assets/
│   ├── css/
│   │   └── styles.css (extracted from inline)
│   ├── js/
│   │   └── explorer.js (extracted from inline)
│   └── images/
│       └── favicon.ico
├── api/
│   └── proxy.php (API proxy for CORS)
└── .htaccess (URL rewriting and security)
```

#### Optimized HTML Structure
```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Qanto Block Explorer - Real-time Blockchain Analytics</title>
    <meta name="description" content="Explore the Qanto blockchain in real-time. View blocks, transactions, balances, and network statistics on the quantum-resistant Layer-0 protocol.">
    <meta name="keywords" content="Qanto, blockchain, explorer, quantum-resistant, cryptocurrency, DAG">
    
    <!-- Open Graph Meta Tags -->
    <meta property="og:title" content="Qanto Block Explorer">
    <meta property="og:description" content="Real-time Qanto blockchain analytics and exploration">
    <meta property="og:type" content="website">
    <meta property="og:url" content="https://explorer.qanto.org">
    <meta property="og:image" content="https://explorer.qanto.org/assets/images/og-image.png">
    
    <!-- Favicon -->
    <link rel="icon" type="image/x-icon" href="/assets/images/favicon.ico">
    
    <!-- Stylesheets -->
    <link rel="stylesheet" href="/assets/css/styles.css">
    
    <!-- Preconnect for performance -->
    <link rel="preconnect" href="https://fonts.googleapis.com">
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
</head>
<body>
    <!-- Content from qanto-explorer.html -->
    <script src="/assets/js/explorer.js"></script>
</body>
</html>
```

### 4. API Proxy Configuration

#### PHP Proxy Script (`api/proxy.php`)
```php
<?php
header('Content-Type: application/json');
header('Access-Control-Allow-Origin: *');
header('Access-Control-Allow-Methods: GET, POST, OPTIONS');
header('Access-Control-Allow-Headers: Content-Type');

// Handle preflight requests
if ($_SERVER['REQUEST_METHOD'] === 'OPTIONS') {
    http_response_code(200);
    exit();
}

$endpoint = $_GET['endpoint'] ?? '';
$nodeUrl = 'http://api.qanto.org:8082'; // Production API endpoint

// Validate endpoint
$allowedEndpoints = ['health', 'info', 'dag', 'balance'];
if (!in_array(explode('/', $endpoint)[0], $allowedEndpoints)) {
    http_response_code(400);
    echo json_encode(['error' => 'Invalid endpoint']);
    exit();
}

// Make API request
$url = $nodeUrl . '/' . $endpoint;
$context = stream_context_create([
    'http' => [
        'timeout' => 10,
        'method' => 'GET'
    ]
]);

$response = file_get_contents($url, false, $context);

if ($response === false) {
    http_response_code(503);
    echo json_encode(['error' => 'Service unavailable']);
} else {
    echo $response;
}
?>
```

### 5. Performance Optimization

#### .htaccess Configuration
```apache
# Enable compression
<IfModule mod_deflate.c>
    AddOutputFilterByType DEFLATE text/plain
    AddOutputFilterByType DEFLATE text/html
    AddOutputFilterByType DEFLATE text/xml
    AddOutputFilterByType DEFLATE text/css
    AddOutputFilterByType DEFLATE application/xml
    AddOutputFilterByType DEFLATE application/xhtml+xml
    AddOutputFilterByType DEFLATE application/rss+xml
    AddOutputFilterByType DEFLATE application/javascript
    AddOutputFilterByType DEFLATE application/x-javascript
</IfModule>

# Browser caching
<IfModule mod_expires.c>
    ExpiresActive on
    ExpiresByType text/css "access plus 1 year"
    ExpiresByType application/javascript "access plus 1 year"
    ExpiresByType image/png "access plus 1 year"
    ExpiresByType image/jpg "access plus 1 year"
    ExpiresByType image/jpeg "access plus 1 year"
    ExpiresByType image/gif "access plus 1 year"
    ExpiresByType image/ico "access plus 1 year"
    ExpiresByType image/svg+xml "access plus 1 year"
</IfModule>

# Security headers
Header always set X-Content-Type-Options nosniff
Header always set X-Frame-Options DENY
Header always set X-XSS-Protection "1; mode=block"
Header always set Strict-Transport-Security "max-age=31536000; includeSubDomains"

# Clean URLs
RewriteEngine On
RewriteCond %{REQUEST_FILENAME} !-f
RewriteCond %{REQUEST_FILENAME} !-d
RewriteRule ^(.*)$ index.html [QSA,L]
```

## Qanto Main Website Deployment

### 1. Hosting Solution: Namecheap VPS

#### VPS Specifications
- **Plan**: Pulsar VPS
- **Resources**:
  - 2 vCPU cores
  - 4GB RAM
  - 120GB SSD storage
  - 3TB bandwidth
  - Full root access
  - cPanel/WHM included

#### Operating System
- **OS**: Ubuntu 22.04 LTS
- **Web Server**: Nginx 1.22+
- **SSL**: Let's Encrypt with auto-renewal
- **Monitoring**: Netdata for real-time monitoring

### 2. Server Configuration

#### Nginx Configuration (`/etc/nginx/sites-available/qanto.org`)
```nginx
server {
    listen 80;
    server_name qanto.org www.qanto.org;
    return 301 https://$server_name$request_uri;
}

server {
    listen 443 ssl http2;
    server_name qanto.org www.qanto.org;
    
    root /var/www/qanto.org/public;
    index index.html;
    
    # SSL Configuration
    ssl_certificate /etc/letsencrypt/live/qanto.org/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/qanto.org/privkey.pem;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers ECDHE-RSA-AES256-GCM-SHA512:DHE-RSA-AES256-GCM-SHA512:ECDHE-RSA-AES256-GCM-SHA384:DHE-RSA-AES256-GCM-SHA384;
    ssl_prefer_server_ciphers off;
    ssl_session_cache shared:SSL:10m;
    ssl_session_timeout 10m;
    
    # Security Headers
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header X-XSS-Protection "1; mode=block" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header Referrer-Policy "no-referrer-when-downgrade" always;
    add_header Content-Security-Policy "default-src 'self' http: https: data: blob: 'unsafe-inline'" always;
    add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;
    
    # Gzip Compression
    gzip on;
    gzip_vary on;
    gzip_min_length 1024;
    gzip_proxied expired no-cache no-store private must-revalidate auth;
    gzip_types text/plain text/css text/xml text/javascript application/x-javascript application/xml+rss application/javascript;
    
    # Static file caching
    location ~* \.(jpg|jpeg|png|gif|ico|css|js|svg|woff|woff2|ttf|eot)$ {
        expires 1y;
        add_header Cache-Control "public, immutable";
    }
    
    # Main location
    location / {
        try_files $uri $uri/ /index.html;
    }
    
    # API proxy (if needed)
    location /api/ {
        proxy_pass http://api.qanto.org:8082/;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

### 3. Deployment Automation

#### GitHub Actions Workflow (`.github/workflows/deploy-website.yml`)
```yaml
name: Deploy Qanto Website

on:
  push:
    branches: [ main ]
    paths: [ 'website/**' ]
  workflow_dispatch:

jobs:
  deploy:
    runs-on: ubuntu-latest
    
    steps:
    - name: Checkout code
      uses: actions/checkout@v4
      
    - name: Setup Node.js
      uses: actions/setup-node@v4
      with:
        node-version: '18'
        cache: 'npm'
        cache-dependency-path: website/package-lock.json
        
    - name: Install dependencies
      run: |
        cd website
        npm ci
        
    - name: Build website
      run: |
        cd website
        npm run build
        
    - name: Deploy to VPS
      uses: appleboy/ssh-action@v1.0.0
      with:
        host: ${{ secrets.VPS_HOST }}
        username: ${{ secrets.VPS_USERNAME }}
        key: ${{ secrets.VPS_SSH_KEY }}
        script: |
          cd /var/www/qanto.org
          git pull origin main
          cp -r website/* public/
          sudo systemctl reload nginx
          
    - name: Purge Cloudflare cache
      uses: jakejarvis/cloudflare-purge-action@master
      env:
        CLOUDFLARE_ZONE: ${{ secrets.CLOUDFLARE_ZONE }}
        CLOUDFLARE_TOKEN: ${{ secrets.CLOUDFLARE_TOKEN }}
```

### 4. Docker Configuration (Alternative)

#### Dockerfile for Production
```dockerfile
FROM nginx:alpine

# Copy website files
COPY website/ /usr/share/nginx/html/

# Copy nginx configuration
COPY nginx.conf /etc/nginx/nginx.conf

# Install certbot for SSL
RUN apk add --no-cache certbot certbot-nginx

# Expose ports
EXPOSE 80 443

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD curl -f http://localhost/ || exit 1

CMD ["nginx", "-g", "daemon off;"]
```

## Domain Management

### 1. DNS Configuration

#### Primary Domain (qanto.org)
```
# A Records
qanto.org.              300    IN    A      [VPS IP]
www.qanto.org.          300    IN    A      [VPS IP]

# Subdomain for explorer
explorer.qanto.org.     300    IN    A      [Shared Hosting IP]

# API subdomain
api.qanto.org.          300    IN    A      [API Server IP]

# Mail records
qanto.org.              300    IN    MX     10 mail.qanto.org.
mail.qanto.org.         300    IN    A      [Mail Server IP]

# SPF Record
qanto.org.              300    IN    TXT    "v=spf1 include:_spf.namecheap.com ~all"

# DKIM Record
default._domainkey.qanto.org. 300 IN TXT "v=DKIM1; k=rsa; p=[DKIM_PUBLIC_KEY]"

# DMARC Record
_dmarc.qanto.org.       300    IN    TXT    "v=DMARC1; p=quarantine; rua=mailto:dmarc@qanto.org"
```

### 2. Cloudflare Integration

#### Nameserver Configuration
1. **Change Nameservers at Namecheap**:
   - Login to Namecheap account
   - Navigate to Domain List
   - Click "Manage" next to qanto.org
   - Change nameservers to Cloudflare:
     - `ava.ns.cloudflare.com`
     - `carter.ns.cloudflare.com`

#### Cloudflare DNS Settings
```
# Root domain
Type: A
Name: qanto.org
Content: [VPS IP]
Proxy: Enabled (Orange cloud)
TTL: Auto

# WWW subdomain
Type: CNAME
Name: www
Content: qanto.org
Proxy: Enabled (Orange cloud)
TTL: Auto

# Explorer subdomain
Type: A
Name: explorer
Content: [Shared Hosting IP]
Proxy: Enabled (Orange cloud)
TTL: Auto

# API subdomain
Type: A
Name: api
Content: [API Server IP]
Proxy: Disabled (Gray cloud)
TTL: Auto
```

#### Cloudflare Page Rules
```
1. Rule: explorer.qanto.org/*
   Settings:
   - Cache Level: Cache Everything
   - Edge Cache TTL: 2 hours
   - Browser Cache TTL: 4 hours

2. Rule: qanto.org/*
   Settings:
   - Cache Level: Cache Everything
   - Edge Cache TTL: 4 hours
   - Browser Cache TTL: 8 hours
   - Always Use HTTPS: On

3. Rule: api.qanto.org/*
   Settings:
   - Cache Level: Bypass
   - Security Level: High
```

## SSL/TLS Configuration

### 1. Cloudflare SSL Settings

#### SSL/TLS Encryption Mode
- **Mode**: Full (Strict)
- **Edge Certificates**: Universal SSL enabled
- **Always Use HTTPS**: Enabled
- **HSTS**: Enabled with max-age 31536000
- **Minimum TLS Version**: 1.2

#### Advanced Certificate Manager
```
Certificate Type: Universal SSL
Hostnames Covered:
- qanto.org
- www.qanto.org
- explorer.qanto.org
- *.qanto.org

Validation Method: HTTP
Certificate Authority: Let's Encrypt
Auto-Renewal: Enabled
```

### 2. Origin Server Certificates

#### Let's Encrypt Setup (VPS)
```bash
# Install Certbot
sudo apt update
sudo apt install certbot python3-certbot-nginx

# Obtain certificate
sudo certbot --nginx -d qanto.org -d www.qanto.org

# Auto-renewal cron job
echo "0 12 * * * /usr/bin/certbot renew --quiet" | sudo crontab -
```

#### Shared Hosting SSL
- **Method**: Namecheap AutoSSL (Let's Encrypt)
- **Coverage**: explorer.qanto.org
- **Auto-renewal**: Enabled through cPanel

## CDN Integration

### 1. Cloudflare CDN Configuration

#### Performance Settings
```
Auto Minify:
- JavaScript: Enabled
- CSS: Enabled
- HTML: Enabled

Brotli Compression: Enabled
Rocket Loader: Enabled
Mirage: Enabled (for images)
Polish: Lossless

Caching:
- Caching Level: Standard
- Browser Cache TTL: 4 hours
- Always Online: Enabled
```

#### Speed Optimizations
```
HTTP/2: Enabled
HTTP/3 (QUIC): Enabled
0-RTT Connection Resumption: Enabled
IPv6 Compatibility: Enabled
Pseudo IPv4: Add header
```

### 2. Additional CDN Features

#### Image Optimization
- **Polish**: Lossless compression for all images
- **Mirage**: Adaptive image loading
- **WebP Conversion**: Automatic format optimization

#### Mobile Optimization
- **Rocket Loader**: Asynchronous JavaScript loading
- **AMP Support**: Accelerated Mobile Pages
- **Mobile Redirect**: Responsive design optimization

## Monitoring and Analytics

### 1. Uptime Monitoring

#### UptimeRobot Configuration
```
Monitor 1:
Type: HTTP(s)
URL: https://qanto.org
Interval: 5 minutes
Timeout: 30 seconds
Alert Contacts: admin@qanto.org, sms:+1234567890

Monitor 2:
Type: HTTP(s)
URL: https://explorer.qanto.org
Interval: 5 minutes
Timeout: 30 seconds
Alert Contacts: admin@qanto.org

Monitor 3:
Type: HTTP(s)
URL: https://api.qanto.org/health
Interval: 1 minute
Timeout: 10 seconds
Alert Contacts: admin@qanto.org, slack:webhook_url
```

### 2. Performance Monitoring

#### Google Analytics 4 Setup
```html
<!-- Google tag (gtag.js) -->
<script async src="https://www.googletagmanager.com/gtag/js?id=G-XXXXXXXXXX"></script>
<script>
  window.dataLayer = window.dataLayer || [];
  function gtag(){dataLayer.push(arguments);}
  gtag('js', new Date());
  gtag('config', 'G-XXXXXXXXXX', {
    page_title: 'Qanto - Quantum-Resistant Blockchain',
    custom_map: {'custom_parameter_1': 'blockchain_interaction'}
  });
</script>
```

#### Cloudflare Analytics
- **Web Analytics**: Privacy-first analytics
- **Performance Insights**: Core Web Vitals tracking
- **Security Events**: Attack pattern analysis
- **Cache Analytics**: Hit ratio optimization

### 3. Error Tracking

#### Sentry Integration
```javascript
import * as Sentry from "@sentry/browser";

Sentry.init({
  dsn: "https://[key]@[project].ingest.sentry.io/[id]",
  environment: "production",
  beforeSend(event) {
    // Filter out non-critical errors
    if (event.exception) {
      const error = event.exception.values[0];
      if (error.type === 'NetworkError') {
        return null; // Don't send network errors
      }
    }
    return event;
  }
});
```

## Backup and Recovery

### 1. Automated Backups

#### VPS Backup Script
```bash
#!/bin/bash
# /usr/local/bin/backup-website.sh

DATE=$(date +%Y%m%d_%H%M%S)
BACKUP_DIR="/backups/website"
WEBSITE_DIR="/var/www/qanto.org"
S3_BUCKET="qanto-backups"

# Create backup directory
mkdir -p $BACKUP_DIR

# Create compressed backup
tar -czf $BACKUP_DIR/website_$DATE.tar.gz -C $WEBSITE_DIR .

# Upload to S3 (using AWS CLI)
aws s3 cp $BACKUP_DIR/website_$DATE.tar.gz s3://$S3_BUCKET/website/

# Keep only last 30 days of local backups
find $BACKUP_DIR -name "website_*.tar.gz" -mtime +30 -delete

# Log backup completion
echo "$(date): Website backup completed - website_$DATE.tar.gz" >> /var/log/backup.log
```

#### Cron Job Setup
```bash
# Daily backup at 2 AM
0 2 * * * /usr/local/bin/backup-website.sh

# Weekly database backup (if applicable)
0 3 * * 0 /usr/local/bin/backup-database.sh
```

### 2. Disaster Recovery Plan

#### Recovery Procedures
1. **VPS Failure**:
   - Provision new VPS from Namecheap
   - Restore from latest S3 backup
   - Update DNS records if IP changed
   - Verify SSL certificates

2. **Shared Hosting Failure**:
   - Re-upload explorer files via cPanel
   - Restore database from backup
   - Test all functionality

3. **DNS Issues**:
   - Switch to backup DNS provider
   - Update nameservers at registrar
   - Monitor propagation

#### Recovery Time Objectives
- **RTO (Recovery Time Objective)**: 4 hours
- **RPO (Recovery Point Objective)**: 24 hours
- **Maximum Downtime**: 6 hours

## Security Considerations

### 1. Web Application Security

#### Security Headers Implementation
```nginx
# Content Security Policy
add_header Content-Security-Policy "default-src 'self'; script-src 'self' 'unsafe-inline' https://www.googletagmanager.com; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; font-src 'self' https://fonts.gstatic.com; img-src 'self' data: https:; connect-src 'self' https://api.qanto.org;" always;

# Prevent clickjacking
add_header X-Frame-Options "SAMEORIGIN" always;

# XSS Protection
add_header X-XSS-Protection "1; mode=block" always;

# MIME type sniffing prevention
add_header X-Content-Type-Options "nosniff" always;

# Referrer Policy
add_header Referrer-Policy "strict-origin-when-cross-origin" always;

# HSTS
add_header Strict-Transport-Security "max-age=31536000; includeSubDomains; preload" always;
```

### 2. Server Hardening

#### Firewall Configuration (UFW)
```bash
# Reset firewall
sudo ufw --force reset

# Default policies
sudo ufw default deny incoming
sudo ufw default allow outgoing

# Allow SSH (change port from default)
sudo ufw allow 2222/tcp

# Allow HTTP and HTTPS
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp

# Enable firewall
sudo ufw enable
```

#### Fail2Ban Configuration
```ini
# /etc/fail2ban/jail.local
[DEFAULT]
bantime = 3600
findtime = 600
maxretry = 3

[nginx-http-auth]
enabled = true
port = http,https
logpath = /var/log/nginx/error.log

[nginx-limit-req]
enabled = true
port = http,https
logpath = /var/log/nginx/error.log
maxretry = 10

[sshd]
enabled = true
port = 2222
logpath = /var/log/auth.log
maxretry = 3
```

### 3. Regular Security Updates

#### Automated Updates Script
```bash
#!/bin/bash
# /usr/local/bin/security-updates.sh

# Update package lists
apt update

# Install security updates only
apt -s upgrade | grep -i security | wc -l > /tmp/security_updates_count

if [ $(cat /tmp/security_updates_count) -gt 0 ]; then
    # Install security updates
    DEBIAN_FRONTEND=noninteractive apt -y upgrade
    
    # Log update
    echo "$(date): $(cat /tmp/security_updates_count) security updates installed" >> /var/log/security-updates.log
    
    # Check if reboot required
    if [ -f /var/run/reboot-required ]; then
        echo "$(date): Reboot required after security updates" >> /var/log/security-updates.log
        # Send notification (implement notification method)
    fi
fi
```

## Maintenance and Updates

### 1. Regular Maintenance Tasks

#### Weekly Maintenance Checklist
- [ ] Review server logs for errors
- [ ] Check SSL certificate expiration
- [ ] Verify backup completion
- [ ] Monitor disk space usage
- [ ] Review security logs
- [ ] Test website functionality
- [ ] Update dependencies if needed

#### Monthly Maintenance Tasks
- [ ] Security audit and penetration testing
- [ ] Performance optimization review
- [ ] CDN cache analysis and optimization
- [ ] Database optimization (if applicable)
- [ ] Backup restoration testing
- [ ] Documentation updates

### 2. Update Procedures

#### Website Updates
1. **Development Environment Testing**
2. **Staging Environment Deployment**
3. **User Acceptance Testing**
4. **Production Deployment**
5. **Post-deployment Verification**
6. **Rollback Plan Preparation**

#### Emergency Update Process
```bash
#!/bin/bash
# Emergency update script

# Create backup before update
/usr/local/bin/backup-website.sh

# Deploy emergency fix
git pull origin hotfix/emergency-fix
cp -r website/* /var/www/qanto.org/public/

# Reload nginx
sudo systemctl reload nginx

# Purge CDN cache
curl -X POST "https://api.cloudflare.com/client/v4/zones/$ZONE_ID/purge_cache" \
     -H "Authorization: Bearer $CF_TOKEN" \
     -H "Content-Type: application/json" \
     --data '{"purge_everything":true}'

# Verify deployment
curl -f https://qanto.org/ || echo "Deployment verification failed"
```

### 3. Monitoring and Alerting

#### Alert Configuration
```yaml
# alerts.yml
alerts:
  - name: "Website Down"
    condition: "http_status != 200"
    threshold: "2 consecutive failures"
    actions:
      - email: "admin@qanto.org"
      - sms: "+1234567890"
      - slack: "#alerts"
  
  - name: "High Response Time"
    condition: "response_time > 5000ms"
    threshold: "5 minutes"
    actions:
      - email: "admin@qanto.org"
  
  - name: "SSL Certificate Expiring"
    condition: "ssl_days_remaining < 30"
    threshold: "immediate"
    actions:
      - email: "admin@qanto.org"
      - slack: "#alerts"
```

---

## Implementation Timeline

### Phase 1: Infrastructure Setup (Week 1)
- [x] Purchase Namecheap hosting services
- [ ] Configure DNS and nameservers
- [ ] Set up Cloudflare CDN
- [ ] Install SSL certificates

### Phase 2: Explorer Deployment (Week 2)
- [ ] Optimize and deploy explorer files
- [ ] Configure API proxy
- [ ] Set up monitoring
- [ ] Performance testing

### Phase 3: Main Website Deployment (Week 3)
- [ ] VPS configuration and hardening
- [ ] Website deployment and optimization
- [ ] CI/CD pipeline setup
- [ ] Security audit

### Phase 4: Monitoring and Optimization (Week 4)
- [ ] Complete monitoring setup
- [ ] Performance optimization
- [ ] Backup system implementation
- [ ] Documentation and training

---

*This deployment strategy ensures robust, scalable, and secure hosting for both Qanto websites using Namecheap's services with industry best practices for performance, security, and reliability.*