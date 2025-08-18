#!/bin/bash

# Qanto Block Explorer - Free Tier Startup Script
# Optimized for minimal resource usage

set -e

echo "🔗 Starting Qanto Block Explorer (Free Tier Edition)..."

# Check if Node.js is installed
if ! command -v node &> /dev/null; then
    echo "❌ Node.js is not installed. Please install Node.js first."
    exit 1
fi

# Check if npm is installed
if ! command -v npm &> /dev/null; then
    echo "❌ npm is not installed. Please install npm first."
    exit 1
fi

# Install dependencies if node_modules doesn't exist
if [ ! -d "node_modules" ]; then
    echo "📦 Installing dependencies..."
    npm install --production --no-optional
fi

# Set environment variables for free-tier optimization
export NODE_ENV=production
export NODE_OPTIONS="--max-old-space-size=128"
export UV_THREADPOOL_SIZE=2

# Start the explorer with resource limits
echo "🚀 Starting block explorer on http://localhost:3001"
echo "📊 Resource limits: 128MB memory, 2 threads"
echo "⏱️  Cache TTL: 30 seconds (free-tier optimized)"
echo ""
echo "Press Ctrl+C to stop the server"
echo ""

# Start the server
node server.js