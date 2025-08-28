#!/bin/sh
set -e

echo "Starting Qanto Website..."

# Test nginx configuration
nginx -t

# Start nginx in foreground
exec nginx -g "daemon off;"