#!/bin/sh
set -e

# Check if nginx is running
if ! pgrep nginx > /dev/null; then
    echo "Nginx is not running"
    exit 1
fi

# Check if the website is accessible
if ! curl -f -s http://localhost/health > /dev/null; then
    echo "Website health check failed"
    exit 1
fi

echo "Health check passed"
exit 0