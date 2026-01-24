#!/bin/bash
# Highper Gateway - Backup Script
# Creates backups of configuration, certificates, and optionally logs

set -euo pipefail

# Configuration
CONFIG_DIR="${CONFIG_DIR:-/etc/highper-gateway}"
LOG_DIR="${LOG_DIR:-/var/log/highper-gateway}"
DATA_DIR="${DATA_DIR:-/var/lib/highper-gateway}"
BACKUP_DIR="${BACKUP_DIR:-/var/lib/highper-gateway/backups}"
RETENTION_DAYS="${RETENTION_DAYS:-30}"
INCLUDE_LOGS="${INCLUDE_LOGS:-false}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

usage() {
    cat << EOF
Usage: $0 [OPTIONS] COMMAND

Backup and restore Highper Gateway configuration

COMMANDS:
    backup              Create a new backup
    restore BACKUP      Restore from a backup file
    list                List available backups
    cleanup             Remove old backups

OPTIONS:
    -h, --help          Show this help message
    -d, --dir DIR       Backup directory (default: $BACKUP_DIR)
    -r, --retention N   Keep backups for N days (default: $RETENTION_DAYS)
    -l, --logs          Include logs in backup
    -c, --config-only   Backup configuration only (no certs)
    -q, --quiet         Suppress output

EXAMPLES:
    $0 backup                      # Create full backup
    $0 backup --logs               # Include logs
    $0 restore backup-2024.tar.gz  # Restore from backup
    $0 list                        # List backups
    $0 cleanup --retention 7       # Keep only 7 days
EOF
    exit 0
}

# Create backup
do_backup() {
    local timestamp
    timestamp=$(date +%Y%m%d-%H%M%S)
    local backup_name="highper-gateway-backup-${timestamp}"
    local backup_file="${BACKUP_DIR}/${backup_name}.tar.gz"
    local temp_dir

    log_info "Creating backup: $backup_name"

    # Create backup directory
    mkdir -p "$BACKUP_DIR"

    # Create temp directory
    temp_dir=$(mktemp -d)
    trap "rm -rf $temp_dir" EXIT

    # Copy configuration
    log_info "Backing up configuration..."
    mkdir -p "$temp_dir/config"
    cp -r "$CONFIG_DIR"/* "$temp_dir/config/" 2>/dev/null || true

    # Create manifest
    cat > "$temp_dir/manifest.json" << EOF
{
  "timestamp": "$(date -Iseconds)",
  "hostname": "$(hostname)",
  "version": "$(highper-gateway --version 2>/dev/null || echo 'unknown')",
  "includes": {
    "config": true,
    "certs": $([ -d "$CONFIG_DIR/certs" ] && echo "true" || echo "false"),
    "logs": $INCLUDE_LOGS
  }
}
EOF

    # Include logs if requested
    if [[ "$INCLUDE_LOGS" == "true" ]]; then
        log_info "Backing up logs..."
        mkdir -p "$temp_dir/logs"
        cp -r "$LOG_DIR"/* "$temp_dir/logs/" 2>/dev/null || true
    fi

    # Create archive
    log_info "Creating archive..."
    tar -czf "$backup_file" -C "$temp_dir" .

    # Calculate checksum
    local checksum
    checksum=$(sha256sum "$backup_file" | cut -d' ' -f1)
    echo "$checksum  $(basename "$backup_file")" > "${backup_file}.sha256"

    log_info "Backup created: $backup_file"
    log_info "Checksum: $checksum"

    # Show size
    local size
    size=$(du -h "$backup_file" | cut -f1)
    log_info "Size: $size"
}

# Restore backup
do_restore() {
    local backup_file="$1"

    # Find backup file
    if [[ ! -f "$backup_file" ]]; then
        if [[ -f "${BACKUP_DIR}/${backup_file}" ]]; then
            backup_file="${BACKUP_DIR}/${backup_file}"
        else
            log_error "Backup file not found: $backup_file"
            exit 1
        fi
    fi

    log_info "Restoring from: $backup_file"

    # Verify checksum if available
    if [[ -f "${backup_file}.sha256" ]]; then
        log_info "Verifying checksum..."
        if ! sha256sum -c "${backup_file}.sha256" &>/dev/null; then
            log_error "Checksum verification failed!"
            exit 1
        fi
        log_info "Checksum OK"
    else
        log_warn "No checksum file found, skipping verification"
    fi

    # Create temp directory
    local temp_dir
    temp_dir=$(mktemp -d)
    trap "rm -rf $temp_dir" EXIT

    # Extract archive
    log_info "Extracting backup..."
    tar -xzf "$backup_file" -C "$temp_dir"

    # Show manifest
    if [[ -f "$temp_dir/manifest.json" ]]; then
        log_info "Backup info:"
        cat "$temp_dir/manifest.json"
        echo ""
    fi

    # Backup current config
    if [[ -d "$CONFIG_DIR" ]]; then
        local current_backup="${BACKUP_DIR}/pre-restore-$(date +%Y%m%d-%H%M%S).tar.gz"
        log_info "Backing up current config to: $current_backup"
        tar -czf "$current_backup" -C "$CONFIG_DIR" .
    fi

    # Stop service
    log_info "Stopping service..."
    systemctl stop highper-gateway 2>/dev/null || true

    # Restore configuration
    log_info "Restoring configuration..."
    if [[ -d "$temp_dir/config" ]]; then
        cp -r "$temp_dir/config"/* "$CONFIG_DIR/"
        chown -R highper-gateway:highper-gateway "$CONFIG_DIR" 2>/dev/null || true
    fi

    # Start service
    log_info "Starting service..."
    systemctl start highper-gateway 2>/dev/null || true

    log_info "Restore complete!"
}

# List backups
do_list() {
    log_info "Available backups in $BACKUP_DIR:"
    echo ""

    if [[ ! -d "$BACKUP_DIR" ]]; then
        log_warn "Backup directory does not exist"
        return
    fi

    local count=0
    while IFS= read -r file; do
        if [[ -n "$file" ]]; then
            local size date
            size=$(du -h "$file" | cut -f1)
            date=$(stat -c %y "$file" | cut -d. -f1)
            printf "  %-45s %8s  %s\n" "$(basename "$file")" "$size" "$date"
            ((count++))
        fi
    done < <(find "$BACKUP_DIR" -name "*.tar.gz" -type f 2>/dev/null | sort -r)

    echo ""
    log_info "Total backups: $count"
}

# Cleanup old backups
do_cleanup() {
    log_info "Cleaning up backups older than $RETENTION_DAYS days..."

    if [[ ! -d "$BACKUP_DIR" ]]; then
        log_warn "Backup directory does not exist"
        return
    fi

    local count=0
    while IFS= read -r file; do
        if [[ -n "$file" ]]; then
            log_info "Removing: $(basename "$file")"
            rm -f "$file"
            rm -f "${file}.sha256"
            ((count++))
        fi
    done < <(find "$BACKUP_DIR" -name "*.tar.gz" -mtime +"$RETENTION_DAYS" -type f 2>/dev/null)

    log_info "Removed $count old backup(s)"
}

# Parse arguments
COMMAND=""
BACKUP_FILE=""
QUIET=false
CONFIG_ONLY=false

while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            usage
            ;;
        -d|--dir)
            BACKUP_DIR="$2"
            shift 2
            ;;
        -r|--retention)
            RETENTION_DAYS="$2"
            shift 2
            ;;
        -l|--logs)
            INCLUDE_LOGS=true
            shift
            ;;
        -c|--config-only)
            CONFIG_ONLY=true
            shift
            ;;
        -q|--quiet)
            QUIET=true
            shift
            ;;
        backup|restore|list|cleanup)
            COMMAND="$1"
            shift
            if [[ "$COMMAND" == "restore" && $# -gt 0 && ! "$1" =~ ^- ]]; then
                BACKUP_FILE="$1"
                shift
            fi
            ;;
        *)
            if [[ -z "$COMMAND" ]]; then
                log_error "Unknown command: $1"
                usage
            fi
            BACKUP_FILE="$1"
            shift
            ;;
    esac
done

# Run command
case $COMMAND in
    backup)
        do_backup
        ;;
    restore)
        if [[ -z "$BACKUP_FILE" ]]; then
            log_error "Backup file required for restore"
            usage
        fi
        do_restore "$BACKUP_FILE"
        ;;
    list)
        do_list
        ;;
    cleanup)
        do_cleanup
        ;;
    *)
        usage
        ;;
esac
