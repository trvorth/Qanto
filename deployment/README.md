# Qanto Deployment - Quick Start

This directory contains deployment configurations for the Qanto blockchain node.

> 📖 **For comprehensive deployment documentation**, see [docs/DEPLOYMENT_GUIDE.md](../docs/DEPLOYMENT_GUIDE.md)

## Directory Structure

```
deployment/
├── docker/                     # Docker deployment (recommended for development)
├── kubernetes/                 # Kubernetes deployment (production)
├── scripts/                    # Deployment automation scripts
└── website/                    # Website and explorer components
```

## Quick Start Options

### 🐳 Docker (Recommended for Development)

```bash
cd deployment/docker
./scripts/deploy.sh
```

See [docker/README.md](docker/README.md) for detailed Docker deployment options.

### ☸️ Kubernetes (Production)

```bash
# Configure and deploy
kubectl apply -f kubernetes/
```

### ☁️ AWS Cloud Deployment

See [README-AWS-DEPLOYMENT.md](../README-AWS-DEPLOYMENT.md) for AWS-specific deployment instructions.

## Prerequisites

- **Docker**: Docker Engine 20.10+ and Docker Compose v2
- **Kubernetes**: kubectl and cluster access (for K8s deployment)
- **Wallet**: Generated wallet file and password
- **Resources**: See [system requirements](../docs/DEPLOYMENT_GUIDE.md#system-requirements)

## Essential Configuration

1. **Generate wallet** (if needed):
```bash
cd deployment/docker
./scripts/generate-wallet.sh
```

2. **Configure environment**:
```bash
cp .env.example .env
# Edit .env with your settings
```

3. **Deploy**:
```bash
./scripts/deploy.sh
```

## Need More Details?

- **Comprehensive Guide**: [docs/DEPLOYMENT_GUIDE.md](../docs/DEPLOYMENT_GUIDE.md)
- **Docker Specifics**: [docker/README.md](docker/README.md)
- **AWS Deployment**: [README-AWS-DEPLOYMENT.md](../README-AWS-DEPLOYMENT.md)
- **Docker Deployment**: [README-DOCKER.md](../README-DOCKER.md)
