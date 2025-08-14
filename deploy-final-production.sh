#!/bin/bash

# Qanto Final Production Deployment Script
# This script builds production Docker images and deploys the entire stack to AWS Free Tier

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Configuration
REGION="us-east-1"
INSTANCE_TYPE="t2.micro"
KEY_NAME="qanto-production-key"
SECURITY_GROUP="qanto-production-sg"
IMAGE_ID="ami-0c02fb55956c7d316" # Amazon Linux 2023
DOCKER_REGISTRY="qanto-registry"

echo -e "${CYAN}🚀 QANTO FINAL PRODUCTION DEPLOYMENT${NC}"
echo -e "${CYAN}====================================${NC}"
echo ""

# Step 1: Build Production Docker Images
echo -e "${YELLOW}📦 Building Production Docker Images...${NC}"

# Build Qanto Node Image
echo -e "${BLUE}Building Qanto Node...${NC}"
cat > Dockerfile.qanto-node << 'EOF'
FROM rustlang/rust:nightly-slim as builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    build-essential \
    cmake \
    clang \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /usr/src/qanto

# Copy source code
COPY . .

# Remove existing lock files and update dependencies
RUN rm -f Cargo.lock myblockchain/Cargo.lock
RUN cargo update

# Build the application in release mode
RUN cargo build --release --bin qanto

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create qanto user
RUN useradd -r -s /bin/false qanto

# Copy binary from builder stage
COPY --from=builder /usr/src/qanto/target/release/qanto /usr/local/bin/qanto

# Create data directory
RUN mkdir -p /var/lib/qanto/data && chown qanto:qanto /var/lib/qanto/data

# Switch to qanto user
USER qanto

# Expose ports
EXPOSE 30333 9933 9944 8080

# Set entrypoint
ENTRYPOINT ["/usr/local/bin/qanto"]
CMD ["--data-dir", "/var/lib/qanto/data", "--rpc-external", "--ws-external", "--rpc-cors", "all"]
EOF

docker build --platform linux/amd64 -f Dockerfile.qanto-node -t qanto-node:production .

# Build Website Image
echo -e "${BLUE}Building Website...${NC}"
cat > website/Dockerfile.production << 'EOF'
FROM nginx:alpine

# Copy website files
COPY index.html /usr/share/nginx/html/

# Create assets directory and copy files if they exist
RUN mkdir -p /usr/share/nginx/html/assets
COPY . /tmp/website/
RUN cp /tmp/website/*.css /usr/share/nginx/html/ 2>/dev/null || true && \
    cp /tmp/website/*.js /usr/share/nginx/html/ 2>/dev/null || true && \
    cp -r /tmp/website/assets/* /usr/share/nginx/html/assets/ 2>/dev/null || true && \
    rm -rf /tmp/website

# Copy nginx configuration
COPY nginx.conf /etc/nginx/nginx.conf

# Expose port
EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]
EOF

# Create nginx configuration
cat > website/nginx.conf << 'EOF'
events {
    worker_connections 1024;
}

http {
    include       /etc/nginx/mime.types;
    default_type  application/octet-stream;
    
    sendfile        on;
    keepalive_timeout  65;
    
    gzip on;
    gzip_vary on;
    gzip_min_length 1024;
    gzip_types text/plain text/css application/json application/javascript text/xml application/xml application/xml+rss text/javascript;
    
    server {
        listen       80;
        server_name  localhost;
        
        location / {
            root   /usr/share/nginx/html;
            index  index.html index.htm;
            try_files $uri $uri/ /index.html;
        }
        
        # API proxy to Qanto node
        location /api/ {
            proxy_pass http://qanto-node:8080/;
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
            proxy_set_header X-Forwarded-Proto $scheme;
        }
        
        # RPC proxy
        location /rpc {
            proxy_pass http://qanto-node:9933;
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
            proxy_set_header X-Forwarded-Proto $scheme;
        }
        
        # WebSocket proxy
        location /ws {
            proxy_pass http://qanto-node:9944;
            proxy_http_version 1.1;
            proxy_set_header Upgrade $http_upgrade;
            proxy_set_header Connection "upgrade";
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
            proxy_set_header X-Forwarded-Proto $scheme;
        }
        
        error_page   500 502 503 504  /50x.html;
        location = /50x.html {
            root   /usr/share/nginx/html;
        }
    }
}
EOF

docker build --platform linux/amd64 -f website/Dockerfile.production -t qanto-website:production website/

echo -e "${GREEN}✅ Docker images built successfully${NC}"

# Step 2: Create AWS Infrastructure
echo -e "${YELLOW}☁️  Setting up AWS Infrastructure...${NC}"

# Check if AWS CLI is installed
if ! command -v aws &> /dev/null; then
    echo -e "${RED}❌ AWS CLI not found. Please install it first.${NC}"
    exit 1
fi

# Create key pair if it doesn't exist
if ! aws ec2 describe-key-pairs --key-names $KEY_NAME --region $REGION &>/dev/null; then
    echo -e "${BLUE}Creating SSH key pair...${NC}"
    aws ec2 create-key-pair --key-name $KEY_NAME --region $REGION --query 'KeyMaterial' --output text > ~/.ssh/${KEY_NAME}.pem
    chmod 400 ~/.ssh/${KEY_NAME}.pem
fi

# Create security group if it doesn't exist
if ! aws ec2 describe-security-groups --group-names $SECURITY_GROUP --region $REGION &>/dev/null; then
    echo -e "${BLUE}Creating security group...${NC}"
    SECURITY_GROUP_ID=$(aws ec2 create-security-group \
        --group-name $SECURITY_GROUP \
        --description "Qanto Production Security Group" \
        --region $REGION \
        --query 'GroupId' --output text)
    
    # Add rules
    aws ec2 authorize-security-group-ingress \
        --group-id $SECURITY_GROUP_ID \
        --protocol tcp \
        --port 22 \
        --cidr 0.0.0.0/0 \
        --region $REGION
    
    aws ec2 authorize-security-group-ingress \
        --group-id $SECURITY_GROUP_ID \
        --protocol tcp \
        --port 80 \
        --cidr 0.0.0.0/0 \
        --region $REGION
    
    aws ec2 authorize-security-group-ingress \
        --group-id $SECURITY_GROUP_ID \
        --protocol tcp \
        --port 30333 \
        --cidr 0.0.0.0/0 \
        --region $REGION
    
    aws ec2 authorize-security-group-ingress \
        --group-id $SECURITY_GROUP_ID \
        --protocol tcp \
        --port 9933 \
        --cidr 0.0.0.0/0 \
        --region $REGION
    
    aws ec2 authorize-security-group-ingress \
        --group-id $SECURITY_GROUP_ID \
        --protocol tcp \
        --port 9944 \
        --cidr 0.0.0.0/0 \
        --region $REGION
else
    SECURITY_GROUP_ID=$(aws ec2 describe-security-groups --group-names $SECURITY_GROUP --region $REGION --query 'SecurityGroups[0].GroupId' --output text)
fi

# Step 3: Launch EC2 Instance
echo -e "${YELLOW}🖥️  Launching EC2 Instance...${NC}"

# Create user data script
cat > user-data.sh << 'EOF'
#!/bin/bash
yum update -y
yum install -y docker
systemctl start docker
systemctl enable docker
usermod -a -G docker ec2-user

# Install Docker Compose
curl -L "https://github.com/docker/compose/releases/latest/download/docker-compose-$(uname -s)-$(uname -m)" -o /usr/local/bin/docker-compose
chmod +x /usr/local/bin/docker-compose

# Create qanto directory
mkdir -p /home/ec2-user/qanto
chown ec2-user:ec2-user /home/ec2-user/qanto
EOF

# Launch instance
INSTANCE_ID=$(aws ec2 run-instances \
    --image-id $IMAGE_ID \
    --count 1 \
    --instance-type $INSTANCE_TYPE \
    --key-name $KEY_NAME \
    --security-group-ids $SECURITY_GROUP_ID \
    --user-data file://user-data.sh \
    --region $REGION \
    --tag-specifications 'ResourceType=instance,Tags=[{Key=Name,Value=Qanto-Production-Node}]' \
    --query 'Instances[0].InstanceId' --output text)

echo -e "${BLUE}Instance ID: $INSTANCE_ID${NC}"

# Wait for instance to be running
echo -e "${BLUE}Waiting for instance to be running...${NC}"
aws ec2 wait instance-running --instance-ids $INSTANCE_ID --region $REGION

# Get public IP
INSTANCE_IP=$(aws ec2 describe-instances \
    --instance-ids $INSTANCE_ID \
    --region $REGION \
    --query 'Reservations[0].Instances[0].PublicIpAddress' --output text)

echo -e "${GREEN}✅ Instance launched successfully at IP: $INSTANCE_IP${NC}"

# Step 4: Deploy Application
echo -e "${YELLOW}🚀 Deploying Application...${NC}"

# Wait for SSH to be available
echo -e "${BLUE}Waiting for SSH to be available...${NC}"
while ! nc -z $INSTANCE_IP 22; do
    sleep 5
done

# Save Docker images
echo -e "${BLUE}Saving Docker images...${NC}"
docker save qanto-node:production | gzip > qanto-node.tar.gz
docker save qanto-website:production | gzip > qanto-website.tar.gz

# Create docker-compose file
cat > docker-compose.production.yml << 'EOF'
version: '3.8'

services:
  qanto-node:
    image: qanto-node:production
    container_name: qanto-node
    ports:
      - "30333:30333"  # P2P
      - "9933:9933"    # RPC
      - "9944:9944"    # WebSocket
      - "8080:8080"    # HTTP API
    volumes:
      - qanto-data:/var/lib/qanto/data
    environment:
      - RUST_LOG=info
    restart: unless-stopped
    networks:
      - qanto-network

  qanto-website:
    image: qanto-website:production
    container_name: qanto-website
    ports:
      - "80:80"
    depends_on:
      - qanto-node
    restart: unless-stopped
    networks:
      - qanto-network

volumes:
  qanto-data:

networks:
  qanto-network:
    driver: bridge
EOF

# Transfer files to instance
echo -e "${BLUE}Transferring files to instance...${NC}"
scp -i ~/.ssh/${KEY_NAME}.pem -o StrictHostKeyChecking=no \
    qanto-node.tar.gz \
    qanto-website.tar.gz \
    docker-compose.production.yml \
    ec2-user@$INSTANCE_IP:/home/ec2-user/qanto/

# Deploy on instance
echo -e "${BLUE}Deploying on instance...${NC}"
ssh -i ~/.ssh/${KEY_NAME}.pem -o StrictHostKeyChecking=no ec2-user@$INSTANCE_IP << 'ENDSSH'
cd /home/ec2-user/qanto

# Install Docker first
echo "Installing Docker..."
# Try different package managers
if command -v yum &> /dev/null; then
    sudo yum update -y
    sudo yum install -y docker
elif command -v dnf &> /dev/null; then
    sudo dnf update -y
    sudo dnf install -y docker
elif command -v apt-get &> /dev/null; then
    sudo apt-get update -y
    sudo apt-get install -y docker.io
else
    echo "No supported package manager found"
    exit 1
fi

# Ensure Docker is running and user has permissions
echo "Setting up Docker permissions..."
sudo systemctl start docker
sudo systemctl enable docker
sudo usermod -a -G docker ec2-user

# Install docker-compose if not present
if ! command -v docker-compose &> /dev/null; then
    echo "Installing docker-compose..."
    # Try pip3 first (more reliable)
    if command -v pip3 &> /dev/null; then
        sudo pip3 install docker-compose
    else
        # Install python3-pip first
        if command -v yum &> /dev/null; then
            sudo yum install -y python3-pip
        elif command -v dnf &> /dev/null; then
            sudo dnf install -y python3-pip
        elif command -v apt-get &> /dev/null; then
            sudo apt-get install -y python3-pip
        fi
        sudo pip3 install docker-compose
    fi
    # Fallback to curl method if pip3 fails
    if ! command -v docker-compose &> /dev/null; then
        sudo curl -L "https://github.com/docker/compose/releases/latest/download/docker-compose-$(uname -s)-$(uname -m)" -o /usr/local/bin/docker-compose
        sudo chmod +x /usr/local/bin/docker-compose
    fi
fi

# Load Docker images with sudo
echo "Loading Docker images..."
sudo docker load < qanto-node.tar.gz
sudo docker load < qanto-website.tar.gz

# Start services with sudo
echo "Starting services..."
sudo /usr/local/bin/docker-compose -f docker-compose.production.yml up -d

# Wait for services to be ready
echo "Waiting for services to be ready..."
sleep 30

# Check status
sudo /usr/local/bin/docker-compose -f docker-compose.production.yml ps
ENDSSH

# Cleanup local files
rm -f qanto-node.tar.gz qanto-website.tar.gz user-data.sh

echo -e "${GREEN}🎉 QANTO PRODUCTION DEPLOYMENT COMPLETE! 🎉${NC}"
echo ""
echo -e "${CYAN}=================================${NC}"
echo -e "${CYAN}     DEPLOYMENT INFORMATION      ${NC}"
echo -e "${CYAN}=================================${NC}"
echo ""
echo -e "${YELLOW}📍 Server Details:${NC}"
echo "   Instance IP:     $INSTANCE_IP"
echo "   Instance ID:     $INSTANCE_ID"
echo "   Region:          $REGION"
echo "   Instance Type:   $INSTANCE_TYPE (Free Tier)"
echo ""
echo -e "${YELLOW}🌐 Network Endpoints:${NC}"
echo "   🏠 Main Website:    http://$INSTANCE_IP/"
echo "   🔗 P2P Node:       $INSTANCE_IP:30333"
echo "   🔧 RPC API:        http://$INSTANCE_IP:9933"
echo "   ⚡ WebSocket:      ws://$INSTANCE_IP:9944"
echo "   📊 Health Check:   http://$INSTANCE_IP/api/health"
echo ""
echo -e "${YELLOW}🛠️  Management:${NC}"
echo "   SSH Access:      ssh -i ~/.ssh/${KEY_NAME}.pem ec2-user@$INSTANCE_IP"
echo "   View Logs:       ssh -i ~/.ssh/${KEY_NAME}.pem ec2-user@$INSTANCE_IP 'cd qanto && /usr/local/bin/docker-compose logs -f'"
echo "   Restart:         ssh -i ~/.ssh/${KEY_NAME}.pem ec2-user@$INSTANCE_IP 'cd qanto && /usr/local/bin/docker-compose restart'"
echo ""
echo -e "${YELLOW}💰 Cost Analysis:${NC}"
echo "   EC2 t2.micro:    \$0.00/month (Free Tier - 750 hours)"
echo "   EBS Storage:     \$0.00/month (Free Tier - 30GB)"
echo "   Data Transfer:   \$0.00/month (Free Tier - 15GB out)"
echo "   ${GREEN}Total Cost:      \$0.00/month${NC}"
echo ""
echo -e "${GREEN}✨ Your Qanto blockchain network is now running 24/7 at ZERO cost! ✨${NC}"
echo ""
echo -e "${PURPLE}🔗 Connect your local node with:${NC}"
echo "   ./target/release/qanto --bootnodes /ip4/$INSTANCE_IP/tcp/30333"
echo ""
echo -e "${CYAN}==============================${NC}"
echo -e "${CYAN}   MISSION ACCOMPLISHED! 🚀   ${NC}"
echo -e "${CYAN}==============================${NC}"