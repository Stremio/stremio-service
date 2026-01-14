#!/bin/bash
# Copyright (C) 2017-2026 Smart Code OOD 203358507
# Docker entrypoint script for stremio-service

set -e

# Colors for logging
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_debug() {
    if [ "${LOG_LEVEL}" = "debug" ]; then
        echo -e "${BLUE}[DEBUG]${NC} $1"
    fi
}

# Create necessary directories
setup_directories() {
    log_info "Setting up directories..."
    mkdir -p "${STREMIO_DATA_DIR:-/data}"
    mkdir -p /var/log/nginx
    mkdir -p /var/log/stremio
    mkdir -p /var/lib/stremio-bans
    mkdir -p /run/nginx
    
    # Create log files
    touch /var/log/nginx/access.log
    touch /var/log/nginx/scanner.log
    touch /var/log/nginx/unmatched.log
    
    # Set permissions
    chown -R nginx:nginx /var/log/nginx
    chmod 755 /var/log/nginx
}

# Setup HTTP Basic Auth if credentials provided
setup_auth() {
    local auth_conf="/etc/nginx/auth.conf"
    local htpasswd_file="/etc/nginx/.htpasswd"
    
    if [ -n "${USERNAME}" ] && [ -n "${PASSWORD}" ]; then
        log_info "Setting up HTTP Basic Authentication..."
        
        # Create htpasswd file
        htpasswd -bc "${htpasswd_file}" "${USERNAME}" "${PASSWORD}"
        chmod 600 "${htpasswd_file}"
        
        # Update auth.conf
        cat > "${auth_conf}" << EOF
auth_basic "Stremio Service";
auth_basic_user_file ${htpasswd_file};
EOF
        log_info "HTTP Basic Authentication configured for user: ${USERNAME}"
    else
        log_info "No authentication configured (USERNAME/PASSWORD not set)"
        # Empty auth config
        echo "# No authentication configured" > "${auth_conf}"
    fi
}

# Setup SSL/HTTPS if certificate is provided
setup_ssl() {
    if [ -n "${CERT_FILE}" ] && [ -f "${CERT_FILE}" ]; then
        log_info "Setting up HTTPS with custom certificate..."
        cp /etc/nginx/http.d/https.conf.template /etc/nginx/http.d/default.conf
        
        # Update certificate paths in config
        sed -i "s|/certs/certificate.pem|${CERT_FILE}|g" /etc/nginx/http.d/default.conf
        
        if [ -n "${KEY_FILE}" ] && [ -f "${KEY_FILE}" ]; then
            sed -i "s|/certs/private.key|${KEY_FILE}|g" /etc/nginx/http.d/default.conf
        fi
        
        log_info "HTTPS configured with certificate: ${CERT_FILE}"
    elif [ -n "${IPADDRESS}" ]; then
        log_warn "IPADDRESS set but no CERT_FILE provided. Running in HTTP mode."
        log_warn "For HTTPS, provide CERT_FILE and optionally KEY_FILE"
    fi
}

# Configure server URL
setup_server_url() {
    if [ -n "${SERVER_URL}" ]; then
        log_info "Server URL configured: ${SERVER_URL}"
        export STREMIO_SERVER_URL="${SERVER_URL}"
    elif [ "${AUTO_SERVER_URL}" = "1" ]; then
        log_info "Auto server URL detection enabled"
    fi
}

# Start the IP banner background process
start_ip_banner() {
    if [ "${ENABLE_IP_BAN}" = "1" ]; then
        log_info "Starting IP banner service..."
        log_info "  - Ban threshold: ${BAN_THRESHOLD:-10} requests"
        log_info "  - Ban window: ${BAN_WINDOW:-60} seconds"
        log_info "  - Ban duration: ${BAN_DURATION:-3600} seconds"
        
        # Start the IP banner in background
        /app/ip-banner.sh &
        IP_BANNER_PID=$!
        log_info "IP banner started (PID: ${IP_BANNER_PID})"
    else
        log_info "IP banning disabled (ENABLE_IP_BAN != 1)"
    fi
}

# Start nginx
start_nginx() {
    log_info "Starting nginx..."
    
    # Test nginx configuration
    if nginx -t 2>&1; then
        log_info "Nginx configuration test passed"
    else
        log_error "Nginx configuration test failed!"
        nginx -t
        exit 1
    fi
    
    # Start nginx in background
    nginx &
    NGINX_PID=$!
    log_info "Nginx started (PID: ${NGINX_PID})"
}

# Start Stremio server
start_stremio() {
    log_info "Starting Stremio Service..."
    
    # Set environment variables for the server
    export HOME="${STREMIO_DATA_DIR:-/data}"
    export FFMPEG_BIN="${FFMPEG_BIN:-/app/ffmpeg}"
    export FFPROBE_BIN="${FFPROBE_BIN:-/app/ffprobe}"
    
    cd /app
    
    # Start the Stremio service
    if [ -f "/app/stremio-service" ]; then
        log_info "Starting stremio-service binary..."
        /app/stremio-service &
        STREMIO_PID=$!
    elif [ -f "/app/server.js" ]; then
        log_info "Starting server.js with Node..."
        node /app/server.js &
        STREMIO_PID=$!
    else
        log_error "No Stremio server binary found!"
        exit 1
    fi
    
    log_info "Stremio Service started (PID: ${STREMIO_PID})"
}

# Graceful shutdown handler
shutdown() {
    log_info "Shutting down..."
    
    # Stop IP banner
    if [ -n "${IP_BANNER_PID}" ]; then
        log_info "Stopping IP banner..."
        kill -TERM "${IP_BANNER_PID}" 2>/dev/null || true
    fi
    
    # Stop nginx
    if [ -n "${NGINX_PID}" ]; then
        log_info "Stopping nginx..."
        nginx -s quit 2>/dev/null || kill -TERM "${NGINX_PID}" 2>/dev/null || true
    fi
    
    # Stop Stremio
    if [ -n "${STREMIO_PID}" ]; then
        log_info "Stopping Stremio Service..."
        kill -TERM "${STREMIO_PID}" 2>/dev/null || true
    fi
    
    log_info "Shutdown complete"
    exit 0
}

# Main
main() {
    log_info "=========================================="
    log_info "   Stremio Service Docker Container"
    log_info "=========================================="
    
    # Setup signal handlers
    trap shutdown SIGTERM SIGINT SIGQUIT
    
    # Run setup functions
    setup_directories
    setup_auth
    setup_ssl
    setup_server_url
    
    # Start services
    start_stremio
    
    # Wait for Stremio to start
    sleep 2
    
    start_nginx
    start_ip_banner
    
    log_info "=========================================="
    log_info "   All services started successfully!"
    log_info "=========================================="
    log_info ""
    log_info "Access the server at: http://localhost:8080"
    if [ -n "${USERNAME}" ]; then
        log_info "Authentication: Enabled (user: ${USERNAME})"
    fi
    log_info ""
    
    # Wait for any process to exit
    wait -n
    
    # If we get here, something crashed
    log_error "A service has crashed unexpectedly"
    shutdown
}

main "$@"
