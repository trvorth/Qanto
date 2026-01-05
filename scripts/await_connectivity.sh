#!/usr/bin/env bash

TARGET_HOST="98.91.191.193"
TARGET_PORT="8080"
SLEEP_SECONDS=30

while true; do
  TIMESTAMP="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

  if nc -z "${TARGET_HOST}" "${TARGET_PORT}" >/dev/null 2>&1; then
    echo "${TIMESTAMP} connectivity to ${TARGET_HOST}:${TARGET_PORT} restored, starting remote certification sequence"

    cargo run --release --bin bench_core -- --json || echo "${TIMESTAMP} bench_core execution failed"

    cargo run --release --bin qanto_cli -- --node "http://${TARGET_HOST}:${TARGET_PORT}" check --seconds 5 \
      || echo "${TIMESTAMP} qanto_cli BPS check failed"

    exit 0
  else
    echo "${TIMESTAMP} connectivity to ${TARGET_HOST}:${TARGET_PORT} unavailable, retrying in ${SLEEP_SECONDS}s"
  fi

  sleep "${SLEEP_SECONDS}"
done
