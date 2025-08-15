# Qanto Website Dockerfile - Static version
FROM nginx:alpine

# Copy static files
COPY . /usr/share/nginx/html/

# Create health check endpoint
RUN mkdir -p /usr/share/nginx/html/api && \
    echo '{"status":"healthy","service":"qanto-website"}' > /usr/share/nginx/html/api/health

# Configure nginx
COPY <<EOF /etc/nginx/nginx.conf
user nginx;
worker_processes auto;
error_log /var/log/nginx/error.log notice;
pid /var/run/nginx.pid;

events {
    worker_connections 1024;
}

http {
    include /etc/nginx/mime.types;
    default_type application/octet-stream;
    
    log_format main '\$remote_addr - \$remote_user [\$time_local] "\$request" '
                    '\$status \$body_bytes_sent "\$http_referer" '
                    '"\$http_user_agent" "\$http_x_forwarded_for"';
    
    access_log /var/log/nginx/access.log main;
    
    sendfile on;
    keepalive_timeout 65;
    gzip on;
    gzip_types text/plain text/css application/json application/javascript text/xml application/xml application/xml+rss text/javascript;
    
    server {
        listen 3000;
        server_name _;
        root /usr/share/nginx/html;
        index index.html;
        
        # Health check
        location /api/health {
            add_header Content-Type application/json;
            return 200 '{"status":"healthy","service":"qanto-website","timestamp":"2024-01-01T00:00:00Z"}';
        }
        
        # Main site
        location / {
            try_files \$uri \$uri/ /index.html;
            add_header X-Content-Type-Options nosniff;
            add_header X-Frame-Options SAMEORIGIN;
        }
        
        # Static assets caching
        location ~* \.(js|css|png|jpg|jpeg|gif|ico|svg|woff|woff2)\$ {
            expires 1y;
            add_header Cache-Control "public, immutable";
        }
    }
}
EOF

EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=10s --start-period=30s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:3000/api/health || exit 1

CMD ["nginx", "-g", "daemon off;"]
