#!/bin/sh
set -eu

: "${QANTO_API_UPSTREAM:=http://rpcnode:8081}"
export QANTO_API_UPSTREAM

envsubst '$QANTO_API_UPSTREAM' < /etc/nginx/templates/default.conf.template > /etc/nginx/conf.d/default.conf

exec nginx -g 'daemon off;'
