#!/bin/bash
# Copyright (C) 2017-2026 Smart Code OOD 203358507
# IP Banner Script - Automatically bans IPs that trigger scanner detection
# Similar to fail2ban but lightweight and tailored for Stremio

set -e

# Configuration (from environment or defaults)
BAN_THRESHOLD="${BAN_THRESHOLD:-10}"       # Number of violations before ban
BAN_WINDOW="${BAN_WINDOW:-60}"             # Time window in seconds
BAN_DURATION="${BAN_DURATION:-3600}"       # Ban duration in seconds (1 hour)
SCANNER_LOG="/var/log/nginx/scanner.log"   # Log file to monitor
BLOCKED_IPS_CONF="/etc/nginx/blocked_ips.conf"
BAN_DB="/var/lib/stremio-bans/bans.db"
VIOLATION_DB="/var/lib/stremio-bans/violations.db"
CHECK_INTERVAL=10                          # How often to check logs (seconds)

# Colors for logging
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() {
    echo -e "${GREEN}[IP-BANNER]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[IP-BANNER]${NC} $1"
}

log_ban() {
    echo -e "${RED}[IP-BANNER]${NC} BANNED: $1"
}

# Initialize database files
init_db() {
    mkdir -p "$(dirname "${BAN_DB}")"
    touch "${BAN_DB}"
    touch "${VIOLATION_DB}"
    
    # Clean up stale entries on startup
    cleanup_expired_bans
}

# Get current timestamp
now() {
    date +%s
}

# Record a violation for an IP
record_violation() {
    local ip="$1"
    local timestamp
    timestamp=$(now)
    
    echo "${ip}|${timestamp}" >> "${VIOLATION_DB}"
}

# Count recent violations for an IP
count_violations() {
    local ip="$1"
    local window_start
    window_start=$(($(now) - BAN_WINDOW))
    
    grep "^${ip}|" "${VIOLATION_DB}" 2>/dev/null | \
        awk -F'|' -v start="${window_start}" '$2 >= start {count++} END {print count+0}'
}

# Check if an IP is already banned
is_banned() {
    local ip="$1"
    grep -q "^${ip}|" "${BAN_DB}" 2>/dev/null
}

# Ban an IP
ban_ip() {
    local ip="$1"
    local ban_until
    ban_until=$(($(now) + BAN_DURATION))
    
    # Skip if already banned
    if is_banned "${ip}"; then
        return 0
    fi
    
    # Add to ban database
    echo "${ip}|${ban_until}" >> "${BAN_DB}"
    
    # Add to nginx blocked_ips.conf
    echo "deny ${ip};  # Banned at $(date -Iseconds) until $(date -d @${ban_until} -Iseconds 2>/dev/null || date -r ${ban_until} -Iseconds 2>/dev/null || echo ${ban_until})" >> "${BLOCKED_IPS_CONF}"
    
    # Reload nginx to apply the ban
    nginx -s reload 2>/dev/null || true
    
    log_ban "${ip} - Banned for ${BAN_DURATION} seconds (${count} violations)"
    
    # Log to syslog/journal if available
    logger -t stremio-ip-banner "Banned IP ${ip} for ${BAN_DURATION} seconds" 2>/dev/null || true
}

# Remove expired bans
cleanup_expired_bans() {
    local current_time
    current_time=$(now)
    local temp_file="${BAN_DB}.tmp"
    local changed=0
    
    # Filter out expired bans
    while IFS='|' read -r ip ban_until; do
        if [ -n "${ip}" ] && [ "${ban_until:-0}" -gt "${current_time}" ]; then
            echo "${ip}|${ban_until}"
        else
            log_info "Unbanning expired IP: ${ip}"
            changed=1
        fi
    done < "${BAN_DB}" > "${temp_file}"
    
    mv "${temp_file}" "${BAN_DB}"
    
    # Clean old violations
    local violation_cutoff=$((current_time - BAN_WINDOW * 2))
    temp_file="${VIOLATION_DB}.tmp"
    
    while IFS='|' read -r ip timestamp; do
        if [ -n "${ip}" ] && [ "${timestamp:-0}" -gt "${violation_cutoff}" ]; then
            echo "${ip}|${timestamp}"
        fi
    done < "${VIOLATION_DB}" > "${temp_file}"
    
    mv "${temp_file}" "${VIOLATION_DB}"
    
    # Rebuild nginx blocked_ips.conf
    if [ "${changed}" = "1" ]; then
        rebuild_blocked_ips
    fi
}

# Rebuild the nginx blocked_ips.conf from the ban database
rebuild_blocked_ips() {
    local current_time
    current_time=$(now)
    
    cat > "${BLOCKED_IPS_CONF}" << 'EOF'
# Copyright (C) 2017-2026 Smart Code OOD 203358507
# Blocked IPs configuration
# This file is automatically managed by ip-banner.sh
# DO NOT EDIT MANUALLY - changes will be overwritten

EOF
    
    while IFS='|' read -r ip ban_until; do
        if [ -n "${ip}" ] && [ "${ban_until:-0}" -gt "${current_time}" ]; then
            echo "deny ${ip};  # Until $(date -d @${ban_until} -Iseconds 2>/dev/null || date -r ${ban_until} 2>/dev/null || echo ${ban_until})"
        fi
    done < "${BAN_DB}" >> "${BLOCKED_IPS_CONF}"
    
    # Reload nginx
    nginx -s reload 2>/dev/null || true
}

# Process the scanner log file
process_scanner_log() {
    local last_position_file="/var/lib/stremio-bans/scanner_log_position"
    local last_position=0
    
    # Read last position
    if [ -f "${last_position_file}" ]; then
        last_position=$(cat "${last_position_file}")
    fi
    
    # Get current file size
    local current_size
    current_size=$(stat -c %s "${SCANNER_LOG}" 2>/dev/null || stat -f %z "${SCANNER_LOG}" 2>/dev/null || echo 0)
    
    # If file was rotated/truncated, start from beginning
    if [ "${current_size}" -lt "${last_position}" ]; then
        last_position=0
    fi
    
    # Process new entries
    if [ "${current_size}" -gt "${last_position}" ]; then
        tail -c +$((last_position + 1)) "${SCANNER_LOG}" 2>/dev/null | while read -r line; do
            # Extract IP from nginx log format
            local ip
            ip=$(echo "${line}" | awk '{print $1}')
            
            if [ -n "${ip}" ] && [[ "${ip}" =~ ^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
                # Skip if already banned
                if is_banned "${ip}"; then
                    continue
                fi
                
                # Record violation
                record_violation "${ip}"
                
                # Check if threshold reached
                local count
                count=$(count_violations "${ip}")
                
                if [ "${count}" -ge "${BAN_THRESHOLD}" ]; then
                    ban_ip "${ip}"
                fi
            fi
        done
        
        # Update position
        echo "${current_size}" > "${last_position_file}"
    fi
}

# Also check unmatched log for suspicious patterns
process_unmatched_log() {
    local unmatched_log="/var/log/nginx/unmatched.log"
    local last_position_file="/var/lib/stremio-bans/unmatched_log_position"
    local last_position=0
    
    if [ ! -f "${unmatched_log}" ]; then
        return 0
    fi
    
    if [ -f "${last_position_file}" ]; then
        last_position=$(cat "${last_position_file}")
    fi
    
    local current_size
    current_size=$(stat -c %s "${unmatched_log}" 2>/dev/null || stat -f %z "${unmatched_log}" 2>/dev/null || echo 0)
    
    if [ "${current_size}" -lt "${last_position}" ]; then
        last_position=0
    fi
    
    if [ "${current_size}" -gt "${last_position}" ]; then
        # Count 404s per IP - more tolerant than scanner log
        tail -c +$((last_position + 1)) "${unmatched_log}" 2>/dev/null | \
            awk '{print $1}' | sort | uniq -c | \
            while read -r count ip; do
                if [ "${count}" -ge $((BAN_THRESHOLD * 3)) ]; then
                    if ! is_banned "${ip}"; then
                        log_warn "IP ${ip} has ${count} unmatched requests"
                        # Record as single violation (less aggressive)
                        record_violation "${ip}"
                    fi
                fi
            done
        
        echo "${current_size}" > "${last_position_file}"
    fi
}

# Print current stats
print_stats() {
    local banned_count
    local violation_count
    
    banned_count=$(wc -l < "${BAN_DB}" 2>/dev/null || echo 0)
    violation_count=$(wc -l < "${VIOLATION_DB}" 2>/dev/null || echo 0)
    
    log_info "Stats: ${banned_count} IPs banned, ${violation_count} recent violations tracked"
}

# Main loop
main() {
    log_info "IP Banner starting..."
    log_info "Configuration:"
    log_info "  - Ban threshold: ${BAN_THRESHOLD} violations"
    log_info "  - Ban window: ${BAN_WINDOW} seconds"
    log_info "  - Ban duration: ${BAN_DURATION} seconds"
    log_info "  - Check interval: ${CHECK_INTERVAL} seconds"
    
    init_db
    
    local iteration=0
    
    while true; do
        # Process logs
        if [ -f "${SCANNER_LOG}" ]; then
            process_scanner_log
        fi
        
        process_unmatched_log
        
        # Cleanup expired bans every 10 iterations
        iteration=$((iteration + 1))
        if [ $((iteration % 10)) -eq 0 ]; then
            cleanup_expired_bans
            print_stats
        fi
        
        sleep "${CHECK_INTERVAL}"
    done
}

# Handle signals
trap 'log_info "Shutting down..."; exit 0' SIGTERM SIGINT SIGQUIT

main "$@"
