#!/bin/bash
# Oxide Installation and Setup Script
# Helps set up Oxide for Linux systems

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Oxide - Setup Script${NC}"
echo -e "${BLUE}========================================${NC}\n"

# Check if running on Linux
if [[ "$OSTYPE" != "linux-gnu"* ]]; then
    echo -e "${RED}❌ This script is for Linux only${NC}"
    exit 1
fi

# Check if running as root
if [[ $EUID -ne 0 ]]; then
    echo -e "${YELLOW}⚠️  This script should be run with sudo${NC}"
    echo -e "${YELLOW}Re-running with sudo...${NC}\n"
    exec sudo "$0" "$@"
fi

echo -e "${GREEN}✅ Running as root${NC}\n"

# Check dependencies
echo -e "${BLUE}Checking dependencies...${NC}"

if ! command -v ip &> /dev/null; then
    echo -e "${RED}❌ 'ip' command not found${NC}"
    echo "Please install iproute2: sudo apt install iproute2"
    exit 1
fi

echo -e "${GREEN}✅ All dependencies found${NC}\n"

# Check if /dev/net/tun exists
if [ ! -e /dev/net/tun ]; then
    echo -e "${YELLOW}⚠️  /dev/net/tun doesn't exist${NC}"
    echo -e "${BLUE}Creating TUN device...${NC}"
    mkdir -p /dev/net
    mknod /dev/net/tun c 10 200
    chmod 0666 /dev/net/tun
    echo -e "${GREEN}✅ TUN device created${NC}\n"
else
    echo -e "${GREEN}✅ TUN device exists${NC}\n"
fi

# Test TUN device
echo -e "${BLUE}Testing TUN device access...${NC}"
if ! [ -r /dev/net/tun ] && [ -w /dev/net/tun ]; then
    echo -e "${RED}❌ Cannot read/write /dev/net/tun${NC}"
    echo "Trying to fix permissions..."
    chmod 0666 /dev/net/tun
fi

echo -e "${GREEN}✅ TUN device is accessible${NC}\n"

# Find and run Oxide
echo -e "${BLUE}Looking for Oxide binary...${NC}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY_PATHS=(
    "$SCRIPT_DIR/target/release/Oxide"
    "$SCRIPT_DIR/target/debug/Oxide"
    "./target/release/Oxide"
    "./target/debug/Oxide"
    "$(which Oxide 2>/dev/null || echo '')"
)

BINARY=""
for path in "${BINARY_PATHS[@]}"; do
    if [ -x "$path" ]; then
        BINARY="$path"
        break
    fi
done

if [ -z "$BINARY" ]; then
    echo -e "${RED}❌ Oxide binary not found${NC}"
    echo -e "${BLUE}Building from source...${NC}"
    
    if ! command -v cargo &> /dev/null; then
        echo -e "${RED}❌ Rust/Cargo not installed${NC}"
        echo "Please install Rust from https://rustup.rs/"
        exit 1
    fi
    
    cd "$SCRIPT_DIR"
    cargo build --release
    BINARY="$SCRIPT_DIR/target/release/Oxide"
fi

echo -e "${GREEN}✅ Found Oxide: $BINARY${NC}\n"

# Run Oxide
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}Starting Oxide...${NC}"
echo -e "${GREEN}========================================${NC}\n"

"$BINARY"
