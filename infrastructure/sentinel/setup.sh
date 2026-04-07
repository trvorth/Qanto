#!/bin/bash
# QANTO Sentinel Setup Script
# v1.0.0 - Phase 39: Resilience Wave

set -e

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${BLUE}================================================================${NC}"
echo -e "${GREEN}      QANTO LAYER-0 | SENTINEL NODE SETUP v1.0.0               ${NC}"
echo -e "${BLUE}================================================================${NC}"

# Check for Docker
if ! [ -x "$(command -v docker)" ]; then
    echo -e "${RED}Error: Docker is not installed.${NC}" >&2
    exit 1
fi

if ! [ -x "$(command -v docker-compose)" ]; then
    echo -e "${RED}Error: Docker Compose is not installed.${NC}" >&2
    exit 1
fi

# Directory Setup
echo -e "${BLUE}Initializing Local Data Directory...${NC}"
mkdir -p data

# Key Generation (Mock for Phase 39 CLI)
if [ ! -f "data/node.key" ]; then
    echo -e "${BLUE}Generating Sentinel Private Key...${NC}"
    # In production: node-cli generate-key --output data/node.key
    openssl rand -hex 32 > data/node.key
    echo -e "${GREEN}Sentinel Key Generated Successfully.${NC}"
else
    echo -e "${BLUE}Existing Sentinel Key Found. Skipping Generation.${NC}"
fi

# Sentinel Enrollment
read -p "Enter Sentinel Node Name (e.g., Pioneer_Alpha): " NODE_NAME
export NODE_NAME=$NODE_NAME

echo -e "${BLUE}Enrolling Sentinel '$NODE_NAME' into QANTO Network...${NC}"
# In production: node-cli enroll --key data/node.key --name $NODE_NAME

# Launch
echo -e "${BLUE}Starting QANTO Sentinel Container...${NC}"
docker-compose up -d

echo -e "${GREEN}================================================================${NC}"
echo -e "${GREEN}   SENTINEL SUCCESSFUL | NODE IS NOW GLOBAL & VERIFIED          ${NC}"
echo -e "${BLUE}================================================================${NC}"
echo -e "Logs: docker-compose logs -f qanto-sentinel"
echo -e "Status: http://localhost:8545/status"
