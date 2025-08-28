# Qanto Unified Website

This directory contains the unified website structure for the Qanto project, supporting both the main website and future block explorer subdomain.

## Structure

```
website/
├── assets/                 # Static assets (CSS, JS, images)
│   ├── css/               # Stylesheets with CSS variables and animations
│   └── js/                # JavaScript modules and visualizations
├── config/                # Configuration files
│   └── domains.json       # Domain and security configuration
├── explorer/              # Block explorer subdomain (placeholder)
│   └── index.html         # Coming soon page
├── .github/               # GitHub Actions workflows
│   └── workflows/         # Deployment automation
├── index.html             # Main website homepage
├── manifest.json          # PWA manifest
├── nginx.conf             # Production nginx configuration
├── Dockerfile             # Multi-stage Docker build
└── health-check.sh        # Container health check script
```

## Domains Supported

### Main Website
- **Primary**: `qanto.org`
- **WWW**: `www.qanto.org`
- **Root**: `/usr/share/nginx/html`
- **Features**: Full Qanto website with PWA support, animations, and optimized assets

### Block Explorer (Future)
- **Subdomain**: `explorer.qanto.org`
- **Root**: `/usr/share/nginx/html/explorer`
- **Status**: Placeholder (returns 503 with "Coming Soon" message)
- **Planned Features**: 
  - Real-time blockchain data visualization
  - Transaction and block search
  - Network statistics and analytics
  - WebSocket connections for live updates

## Features

### Performance Optimizations
- **Compression**: Gzip and Brotli compression enabled
- **Caching**: Aggressive caching for static assets (1 year), shorter for HTML (1 hour)
- **HTTP/2**: Full HTTP/2 support with server push capabilities
- **Asset Optimization**: Minified CSS/JS, optimized images

### Security
- **HTTPS Only**: Automatic HTTP to HTTPS redirects
- **HSTS**: Strict Transport Security with preload
- **CSP**: Content Security Policy headers
- **Security Headers**: X-Frame-Options, X-Content-Type-Options, Referrer-Policy

### Progressive Web App (PWA)
- **Service Worker**: Offline caching and background sync
- **Web App Manifest**: Installable web application
- **Responsive Design**: Mobile-first responsive layout
- **Performance**: Lighthouse score optimization

## Development

### Local Development
```bash
# Serve locally (simple HTTP server)
python3 -m http.server 8000

# Or using Node.js
npx serve .
```

### Docker Development
```bash
# Build the container
docker build -t qanto-website .

# Run locally
docker run -p 80:80 -p 443:443 qanto-website
```

### Production Deployment
The website is automatically deployed via GitHub Actions when changes are pushed to the main branch. The workflow:

1. **Validation**: HTML/CSS/JS validation and testing
2. **Build**: Asset optimization and minification
3. **Security Scan**: Vulnerability scanning
4. **Deploy**: Multi-stage Docker build and deployment
5. **Health Check**: Post-deployment validation

## Configuration

### Domain Configuration
Edit `config/domains.json` to modify:
- Domain settings and SSL paths
- Security headers and CSP policies
- Performance and caching settings

### Nginx Configuration
The `nginx.conf` file includes:
- Multi-domain virtual host configuration
- SSL/TLS settings with modern cipher suites
- Security headers and HSTS
- Compression and caching rules
- Health check endpoints

## Future Enhancements

### Block Explorer Implementation
When implementing the block explorer:

1. **Replace Placeholder**: Remove the 503 response in nginx.conf
2. **Add Explorer App**: Build React/Vue.js application in `explorer/` directory
3. **API Integration**: Connect to Qanto node API endpoints
4. **WebSocket Support**: Add real-time data streaming
5. **Update CSP**: Modify Content Security Policy for explorer requirements

### Additional Subdomains
To add more subdomains (e.g., `docs.qanto.org`, `api.qanto.org`):

1. **Update nginx.conf**: Add new server blocks
2. **Update domains.json**: Add domain configuration
3. **Create Directory Structure**: Add subdomain-specific assets
4. **Update Workflow**: Modify deployment paths in GitHub Actions

## Monitoring and Analytics

### Health Checks
- **Main Site**: `https://qanto.org/health`
- **Explorer**: `https://explorer.qanto.org/health`

### Performance Monitoring
- Lighthouse CI integration in GitHub Actions
- Core Web Vitals tracking
- Real User Monitoring (RUM) ready

### Security Monitoring
- Security header validation
- SSL certificate monitoring
- Vulnerability scanning in CI/CD

## Maintenance

### Regular Tasks
- **SSL Certificate Renewal**: Automated via Let's Encrypt or manual renewal
- **Dependency Updates**: Regular updates to build tools and dependencies
- **Security Patches**: Monitor and apply security updates
- **Performance Optimization**: Regular Lighthouse audits and optimization

### Backup and Recovery
- **Git Repository**: Primary backup via version control
- **Container Registry**: Docker images stored in registry
- **Configuration Backup**: Regular backup of nginx and domain configurations