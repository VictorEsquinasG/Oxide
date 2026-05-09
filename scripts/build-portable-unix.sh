#!/bin/bash

# Build Portable Release for Linux/macOS
# Usage: ./scripts/build-portable-unix.sh [--release] [--zip]

set -e

OUTPUT_DIR="dist"
BUNDLE_PATH="$OUTPUT_DIR/Oxide-Portable"
RELEASE=false
ZIP=false

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --release)
            RELEASE=true
            shift
            ;;
        --zip)
            ZIP=true
            shift
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

echo "🔨 Building portable Oxide bundle..."

# Create output structure
mkdir -p "$BUNDLE_PATH/config"
mkdir -p "$BUNDLE_PATH/assets"

# Determine build profile
if [ "$RELEASE" = true ]; then
    PROFILE_FLAG="--release"
    PROFILE_NAME="release"
else
    PROFILE_FLAG=""
    PROFILE_NAME="debug"
fi

echo -e "\n📦 Building oxide-gui..."
cargo build $PROFILE_FLAG -p oxide-gui

echo -e "\n📦 Building oxide-service..."
cargo build $PROFILE_FLAG -p oxide-service

# Copy binaries
echo -e "\n📁 Copying binaries..."
cp "target/$PROFILE_NAME/oxide-gui" "$BUNDLE_PATH/oxide"
cp "target/$PROFILE_NAME/oxide-service" "$BUNDLE_PATH/oxide-service"

# Copy assets
echo "📁 Copying assets..."
if [ -f "assets/Icon.png" ]; then
    cp "assets/Icon.png" "$BUNDLE_PATH/assets/"
fi

# Create README
echo "📝 Creating portable README..."
cat > "$BUNDLE_PATH/README.txt" << 'EOF'
# Oxide Portable Release

## Quick Start

### Linux / macOS
Open a terminal and run:
```bash
./oxide
```

Or make it executable first:
```bash
chmod +x oxide
./oxide
```

The service daemon will start automatically in the background.

## Troubleshooting

**GUI won't start:**
- Ensure you have graphics support on your system
- Check that your GPU drivers are up-to-date
- For headless systems, use oxide-cli or oxide-service only

**Can't connect to service:**
- Wait a few seconds for oxide-service to initialize
- Check that port 8080 is available on localhost

**Network issues:**
- Ensure all machines can reach each other
- Check firewalls and NAT configurations

## Support

For issues, see the main repository README.

## License

GPL v3
EOF

# Create launcher script
echo "📝 Creating launcher script..."
cat > "$BUNDLE_PATH/launch.sh" << 'EOF'
#!/bin/bash

# Oxide Portable Launcher
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SERVICE_EXE="$SCRIPT_DIR/oxide-service"
GUI_EXE="$SCRIPT_DIR/oxide"

# Make sure executables are marked as executable
chmod +x "$SERVICE_EXE" 2>/dev/null || true
chmod +x "$GUI_EXE" 2>/dev/null || true

# Start service in background
"$SERVICE_EXE" &
SERVICE_PID=$!
echo $SERVICE_PID > /tmp/oxide-service.pid

# Wait for service to initialize
sleep 2

# Launch GUI
"$GUI_EXE"

# Clean up
kill $SERVICE_PID 2>/dev/null || true
rm -f /tmp/oxide-service.pid

exit 0
EOF

chmod +x "$BUNDLE_PATH/launch.sh"

echo -e "\n✅ Portable bundle created at: $BUNDLE_PATH"

# Optional: Create ZIP archive
if [ "$ZIP" = true ]; then
    echo -e "\n📦 Creating compressed archive..."
    
    if command -v zip &> /dev/null; then
        cd "$OUTPUT_DIR"
        zip -r "Oxide-Portable.zip" "Oxide-Portable/" -q
        cd ..
        echo "✅ ZIP archive created: $OUTPUT_DIR/Oxide-Portable.zip"
    else
        echo "⚠️  zip command not found. Install zip package to create archives."
    fi
fi

echo -e "\n🎉 Build complete!"
