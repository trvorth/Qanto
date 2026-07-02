# syntax=docker/dockerfile:1.7

FROM node:20-alpine AS deps
WORKDIR /app
COPY package.json package-lock.json ./
RUN --mount=type=cache,target=/root/.npm npm ci

FROM node:20-alpine AS builder
WORKDIR /app
COPY --from=deps /app/node_modules ./node_modules
COPY . .
ARG VITE_QANTO_NODE_URL=/api
ENV VITE_QANTO_NODE_URL=${VITE_QANTO_NODE_URL}
RUN npm run lint && npm run build

FROM nginx:1.28-alpine AS runtime
RUN apk add --no-cache curl gettext libcap && \
    setcap "cap_net_bind_service=+ep" /usr/sbin/nginx && \
    mkdir -p /etc/nginx/templates /var/cache/nginx /var/run /tmp/nginx && \
    chown -R nginx:nginx /var/cache/nginx /var/run /tmp/nginx /etc/nginx/templates /usr/share/nginx/html && \
    apk del libcap

COPY nginx.conf /etc/nginx/nginx.conf
COPY .docker/default.conf.template /etc/nginx/templates/default.conf.template
COPY .docker/entrypoint.sh /entrypoint.sh
COPY --from=builder /app/dist /usr/share/nginx/html

RUN chmod 755 /entrypoint.sh

USER nginx
ENV QANTO_API_UPSTREAM=http://rpcnode:8081

EXPOSE 80

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -fsS http://127.0.0.1/healthz || exit 1

ENTRYPOINT ["/entrypoint.sh"]
