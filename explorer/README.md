# Qanto Block Explorer - Free Tier Edition

A lightweight, resource-optimized block explorer for the Qanto blockchain, specifically designed for AWS Free Tier deployments.

## Features

- **Minimal Resource Usage**: Optimized for low-memory environments (128MB)
- **Caching System**: 30-second TTL cache to reduce RPC calls
- **Responsive Design**: Mobile-friendly interface
- **Real-time Updates**: Auto-refresh every 30 seconds
- **Free-Tier Optimized**: Designed for AWS t2.nano instances

## Quick Start

### Prerequisites

- Node.js (v14 or higher)
- npm
- Running Qanto node with RPC enabled on port 8082

### Installation

```bash
# Clone or navigate to the explorer directory
cd explorer

# Install dependencies
npm install --production --no-optional

# Start the explorer
./start.sh
```

Or manually:

```bash
node server.js
```

### Access

Open your browser and navigate to:
- **Local**: http://localhost:3000
- **Production**: http://your-server-ip:3000

## Configuration

### Environment Variables

- `PORT`: Server port (default: 3000)
- `QANTO_RPC_URL`: Qanto RPC endpoint (default: http://localhost:8082)
- `CACHE_TTL`: Cache time-to-live in seconds (default: 30)
- `MAX_CACHE_SIZE`: Maximum cache entries (default: 100)

### Free-Tier Optimizations

- **Memory Limit**: 128MB Node.js heap size
- **Thread Pool**: Limited to 2 threads
- **Cache Strategy**: Short TTL to balance performance and memory
- **Request Timeout**: 30 seconds to prevent hanging
- **Minimal Dependencies**: Only essential packages included

## API Endpoints

### Health Check
```
GET /api/health
```
Returns server health status.

### Node Status
```
GET /api/status
```
Returns Qanto node connection status and basic info.

### Recent Blocks
```
GET /api/blocks?limit=10
```
Returns list of recent blocks (default limit: 10).

### Block Details
```
GET /api/block/:hash
```
Returns detailed information for a specific block.

### Transaction Details
```
GET /api/transaction/:hash
```
Returns detailed information for a specific transaction.

### Network Statistics
```
GET /api/stats
```
Returns aggregated network statistics.

## Architecture

### Frontend
- Pure HTML/CSS/JavaScript (no frameworks)
- Responsive design with CSS Grid
- Minimal JavaScript for API calls
- Auto-refresh functionality

### Backend
- Express.js server
- In-memory caching with LRU eviction
- Axios for RPC communication
- CORS enabled for cross-origin requests

### Caching Strategy
- **TTL**: 30 seconds for all cached data
- **Size Limit**: 100 entries maximum
- **LRU Eviction**: Least recently used items removed first
- **Memory Efficient**: Automatic cleanup of expired entries

## Deployment

### Docker (Recommended)

```bash
# Build the image
docker build -f ../Dockerfile.freetier -t qanto-explorer .

# Run with resource limits
docker run -d \
  --name qanto-explorer \
  --memory=200m \
  --cpus=0.2 \
  -p 3000:3000 \
  -e QANTO_RPC_URL=http://qanto-node:8082 \
  qanto-explorer
```

### AWS Free Tier

1. Deploy using the provided Terraform configuration
2. The explorer will be automatically configured and started
3. Access via the EC2 instance's public IP on port 3000

### Manual Deployment

```bash
# Install PM2 for process management
npm install -g pm2

# Start with PM2
pm2 start server.js --name qanto-explorer --max-memory-restart 150M

# Save PM2 configuration
pm2 save
pm2 startup
```

## Monitoring

### Health Checks

The explorer includes built-in health checks:
- Server responsiveness
- RPC connection status
- Memory usage monitoring
- Cache performance metrics

### Logs

Logs are written to stdout and can be captured using:
```bash
# View logs
pm2 logs qanto-explorer

# Or with Docker
docker logs qanto-explorer
```

## Troubleshooting

### Common Issues

1. **RPC Connection Failed**
   - Ensure Qanto node is running
   - Check RPC port (default: 8082)
   - Verify firewall settings

2. **High Memory Usage**
   - Reduce cache size: `MAX_CACHE_SIZE=50`
   - Increase cache TTL: `CACHE_TTL=60`
   - Restart the service

3. **Slow Response Times**
   - Check Qanto node performance
   - Verify network connectivity
   - Monitor cache hit rates

### Performance Tuning

```bash
# Monitor memory usage
ps aux | grep node

# Check cache statistics
curl http://localhost:3000/api/health

# Monitor RPC response times
curl -w "@curl-format.txt" http://localhost:3000/api/status
```

## Development

### Local Development

```bash
# Install development dependencies
npm install

# Start in development mode
npm run dev

# Run tests (if available)
npm test
```

### Code Structure

```
explorer/
├── server.js          # Main server file
├── package.json       # Dependencies and scripts
├── start.sh          # Startup script
├── public/           # Static files
│   └── index.html    # Frontend interface
└── README.md         # This file
```

## Security Considerations

- No authentication required (read-only data)
- CORS enabled for public access
- Rate limiting through caching
- No sensitive data exposure
- Minimal attack surface

## License

This project is part of the Qanto blockchain ecosystem.

## Support

For issues and questions:
1. Check the troubleshooting section
2. Review server logs
3. Verify Qanto node connectivity
4. Check resource usage and limits