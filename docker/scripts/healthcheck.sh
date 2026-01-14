#!/bin/sh
# Copyright (C) 2017-2026 Smart Code OOD 203358507
# Health check script for Docker container

# Check if nginx is running
if ! pgrep -x nginx > /dev/null 2>&1; then
    echo "FAIL: nginx is not running"
    exit 1
fi

# Check if Stremio server is responding
response=$(curl -sf -o /dev/null -w "%{http_code}" http://127.0.0.1:11470/ 2>/dev/null || echo "000")

if [ "$response" = "200" ] || [ "$response" = "401" ] || [ "$response" = "301" ] || [ "$response" = "302" ] || [ "$response" = "307" ]; then
    echo "OK: Stremio server is healthy (HTTP ${response})"
    exit 0
else
    echo "FAIL: Stremio server returned HTTP ${response}"
    exit 1
fi
