# Qanto Docker Deployment

This directory contains a complete Docker-based deployment solution for the Qanto blockchain node with comprehensive monitoring, logging, and management capabilities.

## 🏗️ Architecture Overview

The deployment consists of the following components:

### Core Services
- **Qanto Node**: The main blockchain node with API and P2P networking
- **Qanto Wallet**: Integrated wallet functionality

### Monitoring Stack (Optional)
- **Prometheus**: Metrics collection and storage
- **Grafana**: Visualization and dashboards
- **Loki**: Log aggregation and storage
- **Promtail**: Log collection agent

### Key Features
- 🔒 **Security**: Non-root containers, capability dropping, secrets management
- 📊 **Monitoring**: Comprehensive metrics and logging
- 🔄 **Health Checks**: Automated health monitoring
- 💾 **Backup**: Automated backup and recovery
- 🚀 **Performance**: Optimized resource usage and caching
- 🔧 **Management**: Easy deployment and maintenance scripts

## 📁 Directory Structure

```
deployment/docker/
├── docker-compose.yml          # Main compose configuration
├── Dockerfile                  # Qanto node container definition
├── .env.example               # Environment variables template
├── README.md                  # This file
├── monitoring/                # Monitoring configurations
│   ├── prometheus/
│   │   └── prometheus.yml     # Prometheus configuration
│   ├── grafana/
│   │   ├── provisioning/      # Grafana provisioning
│   │   └── dashboards/        # Pre-built dashboards
│   ├── loki/
│   │   └── local-config.yaml  # Loki configuration
│   └── promtail/
│       └── config.yml         # Promtail configuration
├── scripts/                   # Management scripts
│   ├── deploy.sh             # Main deployment script
│   ├── healthcheck.sh        # Health monitoring
│   └── backup.sh             # Backup and recovery
├── data/                     # Blockchain data (created at runtime)
├── keys/                     # Cryptographic keys (created at runtime)
├── logs/                     # Application logs (created at runtime)
└── backups/                  # Backup storage (created at runtime)
```

## 🚀 Quick Start

### Prerequisites

- Docker Engine 20.10+
- Docker Compose 2.0+
- At least 10GB free disk space
- 4GB RAM minimum (8GB recommended)
- Linux/macOS (Windows with WSL2)

### 1. Initial Setup

```bash
# Clone the repository (if not already done)
cd qanto/deployment/docker

# Copy environment template
cp .env.example .env

# Edit environment variables
vim .env  # or your preferred editor
```

### 2. Deploy Qanto Node Only

```bash
# Deploy just the Qanto node
./scripts/deploy.sh

# Or with explicit profile
DEPLOYMENT_PROFILE=qanto ./scripts/deploy.sh
```

### 3. Deploy with Full Monitoring Stack

```bash
# Deploy with monitoring services
DEPLOYMENT_PROFILE=full ./scripts/deploy.sh
```

### 4. Verify Deployment

```bash
# Check service status
docker-compose ps

# Run health checks
./scripts/healthcheck.sh

# View logs
docker-compose logs -f qanto-node
```

## ⚙️ Configuration

### Environment Variables

Key configuration options in `.env`:

```bash
# Qanto Configuration
QANTO_VERSION=latest
QANTO_ENVIRONMENT=production
QANTO_NODE_NAME=qanto-production-node
QANTO_NETWORK=mainnet

# Network Ports
QANTO_API_PORT=8081
QANTO_P2P_PORT=8333
QANTO_RPC_PORT=8080

# Resource Limits
QANTO_CPU_LIMIT=2.0
QANTO_MEMORY_LIMIT=4g
QANTO_MEMORY_RESERVATION=2g

# Performance Tuning
QANTO_MAX_CONNECTIONS=100
QANTO_CACHE_SIZE=1000
QANTO_WORKER_THREADS=4

# Monitoring
PROMETHEUS_PORT=9090
GRAFANA_PORT=3000
GRAFANA_ADMIN_PASSWORD=secure_password_here
LOKI_PORT=3100

# Storage Paths
QANTO_DATA_PATH=./data
QANTO_KEYS_PATH=./keys
QANTO_LOGS_PATH=./logs
MONITORING_DATA_PATH=./monitoring/data

# Network
DOCKER_SUBNET=172.20.0.0/16
```

### Deployment Profiles

#### `qanto` Profile (Default)
- Qanto node only
- Minimal resource usage
- Basic health checks
- Suitable for production nodes

#### `full` Profile
- Qanto node + monitoring stack
- Comprehensive observability
- Resource intensive
- Suitable for development/monitoring

## 🔧 Management Scripts

### Deployment Script (`scripts/deploy.sh`)

```bash
# Full deployment
./scripts/deploy.sh

# Update services
./scripts/deploy.sh update

# Force rebuild
./scripts/deploy.sh rebuild

# Stop services
./scripts/deploy.sh stop

# Clean up everything
./scripts/deploy.sh clean
```

### Health Check Script (`scripts/healthcheck.sh`)

```bash
# Run health checks
./scripts/healthcheck.sh

# Quiet mode (log only)
./scripts/healthcheck.sh --quiet

# Verbose mode
./scripts/healthcheck.sh --verbose
```

### Backup Script (`scripts/backup.sh`)

```bash
# Full backup
./scripts/backup.sh

# Data only backup
./scripts/backup.sh data

# Configuration backup
./scripts/backup.sh config

# Logs backup
./scripts/backup.sh logs
```

## 📊 Monitoring and Observability

### Service URLs (Full Profile)

- **Qanto Node API**: http://localhost:8081
- **Prometheus**: http://localhost:9090
- **Grafana**: http://localhost:3000 (admin/admin)
- **Loki**: http://localhost:3100

### Pre-built Dashboards

1. **Qanto Node Overview**: Blockchain metrics, peers, transactions
2. **System Overview**: CPU, memory, disk, network usage
3. **Docker Containers**: Container resource usage and health

### Key Metrics

- Blockchain height and sync status
- Transaction throughput (TPS)
- Connected peers
- Memory and CPU usage
- Network I/O
- Error rates and response times

### Log Aggregation

Logs are automatically collected from:
- Qanto node application logs
- Docker container logs
- System logs
- Performance metrics

## 🔒 Security

### Container Security
- Non-root user execution
- Capability dropping
- Read-only root filesystem where possible
- Security options (no-new-privileges)
- Resource limits and ulimits

### Network Security
- Custom Docker network with subnet isolation
- Port exposure only where necessary
- Internal service communication

### Secrets Management
- Docker secrets for sensitive data
- Environment variable isolation
- Secure key storage

### File Permissions
- Restricted access to keys directory (700)
- Proper ownership of data directories
- Log rotation and cleanup

## 💾 Backup and Recovery

### Automated Backups

```bash
# Configure backup environment variables
export BACKUP_DIR="/path/to/backups"
export RETENTION_DAYS=30
export S3_BACKUP_BUCKET="my-qanto-backups"
export BACKUP_ENCRYPTION_KEY="my-secure-passphrase"

# Run backup
./scripts/backup.sh
```

### Backup Components
- Blockchain data and state
- Configuration files
- Cryptographic keys and secrets
- Application logs
- Monitoring data

### Recovery Process

1. Stop services: `./scripts/deploy.sh stop`
2. Restore data from backup
3. Verify configuration
4. Start services: `./scripts/deploy.sh`
5. Run health checks: `./scripts/healthcheck.sh`

## 🔍 Troubleshooting

### Common Issues

#### Services Won't Start
```bash
# Check Docker daemon
docker info

# Check compose file syntax
docker-compose config

# Check resource usage
docker system df
docker system prune
```

#### Health Checks Failing
```bash
# Check service logs
docker-compose logs qanto-node

# Check container status
docker-compose ps

# Manual health check
curl http://localhost:8081/health
```

#### Performance Issues
```bash
# Check resource usage
docker stats

# Check system resources
htop
df -h

# Adjust resource limits in .env
vim .env
```

#### Network Issues
```bash
# Check port bindings
netstat -tlnp | grep -E '(8080|8081|8333)'

# Check Docker networks
docker network ls
docker network inspect qanto-network
```

### Log Locations

- **Deployment logs**: `logs/deploy.log`
- **Health check logs**: `logs/healthcheck.log`
- **Backup logs**: `logs/backup.log`
- **Application logs**: `logs/qanto/`
- **Container logs**: `docker-compose logs [service]`

### Debug Mode

```bash
# Enable debug logging
export QANTO_LOG_LEVEL=debug

# Verbose deployment
./scripts/deploy.sh --verbose

# Container debugging
docker-compose exec qanto-node bash
```

## 🔄 Maintenance

### Regular Tasks

```bash
# Weekly health check
./scripts/healthcheck.sh

# Weekly backup
./scripts/backup.sh

# Monthly cleanup
docker system prune -f

# Update images
./scripts/deploy.sh update
```

### Log Rotation

Logs are automatically rotated using Docker's logging driver:
- Maximum file size: 100MB
- Maximum files: 5
- Automatic compression

### Resource Monitoring

```bash
# Monitor resource usage
watch docker stats

# Check disk usage
du -sh data/ logs/ monitoring/

# Monitor system resources
htop
iotop
```

## 🚀 Performance Tuning

### Resource Optimization

1. **CPU**: Adjust `QANTO_CPU_LIMIT` based on available cores
2. **Memory**: Set `QANTO_MEMORY_LIMIT` to 50-70% of available RAM
3. **Storage**: Use SSD for data directory
4. **Network**: Optimize `QANTO_MAX_CONNECTIONS` for network capacity

### Docker Optimization

```bash
# Enable Docker BuildKit
export DOCKER_BUILDKIT=1

# Use multi-stage builds
# (Already implemented in Dockerfile)

# Optimize image layers
# (Already optimized)
```

### System Optimization

```bash
# Increase file descriptor limits
echo "* soft nofile 65536" >> /etc/security/limits.conf
echo "* hard nofile 65536" >> /etc/security/limits.conf

# Optimize kernel parameters
echo "net.core.somaxconn = 65536" >> /etc/sysctl.conf
echo "net.ipv4.tcp_max_syn_backlog = 65536" >> /etc/sysctl.conf
sysctl -p
```

## 🤝 Contributing

### Development Setup

```bash
# Use development profile
export QANTO_ENVIRONMENT=development
export QANTO_LOG_LEVEL=debug

# Deploy with monitoring
DEPLOYMENT_PROFILE=full ./scripts/deploy.sh
```

### Testing Changes

```bash
# Validate compose file
docker-compose config

# Test build
docker-compose build

# Run health checks
./scripts/healthcheck.sh
```

### Adding New Services

1. Add service definition to `docker-compose.yml`
2. Update monitoring configuration if needed
3. Add health checks to `scripts/healthcheck.sh`
4. Update backup script if persistent data
5. Document in this README

## 📚 Additional Resources

- [Docker Compose Documentation](https://docs.docker.com/compose/)
- [Prometheus Documentation](https://prometheus.io/docs/)
- [Grafana Documentation](https://grafana.com/docs/)
- [Loki Documentation](https://grafana.com/docs/loki/)
- [Qanto Documentation](../../README.md)

## 📄 License

This deployment configuration is part of the Qanto project and follows the same license terms.

---

**Need Help?** Check the troubleshooting section above or review the logs in the `logs/` directory.