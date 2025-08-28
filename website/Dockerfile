# Multi-stage build for Qanto Website
# Stage 1: Build and optimize assets
FROM node:18-alpine AS builder

# Set working directory
WORKDIR /app

# Install build dependencies
RUN apk add --no-cache \
    python3 \
    make \
    g++ \
    imagemagick \
    optipng \
    jpegoptim

# Copy package files
COPY package*.json ./

# Install dependencies
RUN npm ci --only=production

# Install build tools globally
RUN npm install -g \
    terser \
    clean-css-cli \
    html-minifier-terser \
    imagemin-cli \
    @imagemin/pngquant \
    @imagemin/mozjpeg \
    @imagemin/svgo

# Copy source files
COPY . .

# Build and optimize assets
RUN echo "Building and optimizing assets..." && \
    # Create optimized directory
    mkdir -p /app/dist && \
    cp -r . /app/dist/ && \
    cd /app/dist && \
    \
    # Optimize HTML files
    find . -name "*.html" -exec html-minifier-terser \
        --collapse-whitespace \
        --remove-comments \
        --remove-optional-tags \
        --remove-redundant-attributes \
        --remove-script-type-attributes \
        --remove-tag-whitespace \
        --use-short-doctype \
        --minify-css true \
        --minify-js true \
        --output {} {} \; && \
    \
    # Optimize CSS files
    find assets/css -name "*.css" -exec cleancss -o {} {} \; && \
    \
    # Optimize JavaScript files
    find assets/js -name "*.js" -exec terser {} -o {} --compress --mangle \; && \
    \
    # Optimize images if they exist
    if [ -d "assets/images" ]; then \
        find assets/images -name "*.png" -exec optipng -o7 {} \; ; \
        find assets/images -name "*.jpg" -o -name "*.jpeg" -exec jpegoptim --max=85 --strip-all {} \; ; \
        find assets/images -name "*.svg" -exec imagemin {} --out-dir=assets/images --plugin=@imagemin/svgo \; ; \
    fi && \
    \
    # Generate cache manifest
    echo "Generating cache manifest..." && \
    echo '{"version":"'$(date +%s)'","timestamp":"'$(date -u +%Y-%m-%dT%H:%M:%SZ)'","files":[]}' > cache-manifest.json && \
    # Remove unnecessary files
    rm -rf node_modules package*.json Dockerfile* .git* .env* && \
    \
    # Set proper permissions
    find . -type f -exec chmod 644 {} \; && \
    find . -type d -exec chmod 755 {} \;

# Stage 2: Production server
FROM nginx:1.25-alpine AS production

# Install additional tools
RUN apk add --no-cache \
    curl \
    jq \
    tzdata \
    logrotate

# Set timezone
ENV TZ=UTC
RUN ln -snf /usr/share/zoneinfo/$TZ /etc/localtime && echo $TZ > /etc/timezone

# Create nginx user and directories
RUN addgroup -g 1001 -S nginx-app && \
    adduser -S -D -H -u 1001 -h /var/cache/nginx -s /sbin/nologin -G nginx-app -g nginx nginx-app

# Create necessary directories
RUN mkdir -p /var/www/qanto.network \
             /var/log/nginx \
             /var/cache/nginx \
             /etc/nginx/conf.d \
             /etc/ssl/certs \
             /run/nginx

# Copy optimized website files
COPY --from=builder --chown=nginx-app:nginx-app /app/dist /var/www/qanto.network/

# Copy nginx configuration
COPY --chown=root:root nginx.conf /etc/nginx/nginx.conf
COPY --chown=root:root nginx-site.conf /etc/nginx/conf.d/default.conf

# Copy nginx site configuration
COPY nginx-qanto.conf /etc/nginx/conf.d/qanto.conf

# Copy custom error pages
COPY 404.html /var/www/qanto.network/404.html
COPY 50x.html /var/www/qanto.network/50x.html

# Copy health check script
COPY health-check.sh /usr/local/bin/health-check.sh
RUN chmod +x /usr/local/bin/health-check.sh

# Copy startup script
COPY start.sh /usr/local/bin/start.sh
RUN chmod +x /usr/local/bin/start.sh

# Set proper ownership
RUN chown -R nginx-app:nginx-app /var/www/qanto.network /var/cache/nginx /var/log/nginx

# Expose port
EXPOSE 80

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD /usr/local/bin/health-check.sh

# Use non-root user
USER nginx-app

# Start the application
CMD ["/usr/local/bin/start.sh"]

# Labels for metadata
LABEL maintainer="Qanto Team <dev@qanto.network>" \
      version="1.0.0" \
      description="Qanto Network Main Website" \
      org.opencontainers.image.title="Qanto Website" \
      org.opencontainers.image.description="Official Qanto Network website" \
      org.opencontainers.image.vendor="Qanto Network" \
      org.opencontainers.image.licenses="MIT" \
      org.opencontainers.image.source="https://github.com/qanto-network/qanto" \
      org.opencontainers.image.documentation="https://docs.qanto.network"