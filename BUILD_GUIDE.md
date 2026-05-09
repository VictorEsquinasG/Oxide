# Oxide Build & Deployment Guide

## Overview

This guide explains how to build, test, and deploy Oxide for different use cases.

## Architecture

### Rendering Backend Migration (v0.2.0)

**Previous**: OpenGL via `egui_glow`
- ❌ Requires OpenGL 2.0+ drivers
- ❌ Limited compatibility in headless/WSL/Docker environments
- ❌ Not optimal for modern graphics stacks

**Current**: Vulkan/DirectX/Metal via `wgpu`
- ✅ Modern graphics abstraction layer
- ✅ Better Windows 10/11 support (DirectX 12 backend)
- ✅ Better Linux support (Vulkan backend)
- ✅ Fallback to OpenGL available but not primary
- ✅ Easier deployment without strict driver requirements

### Component Stack

```
Oxide Portable Release
├── oxide-gui (wxWidget-like simple UI using egui + wgpu)
├── oxide-service (Background daemon handling networking)
├── oxide-cli (Command-line interface via IPC)
└── oxide-core (Shared networking/room management library)
```

#### oxide-gui → oxide-service Communication

- **Protocol**: JSON over TCP
- **Port**: 127.0.0.1:8080 (localhost only)
- **Behavior**: GUI automatically connects to service; service starts on first GUI launch

#### Cross-Crate Dependencies

```
oxide-gui     → oxide-core (room/network logic)
oxide-cli     → oxide-core (room/network logic)
oxide-service → oxide-core (room/network logic)
```

## Development Workflow

### Local Development (Debug Mode)

#### Terminal 1: Start Service
```bash
cargo run -p oxide-service
```
Output: `Listening on 127.0.0.1:8080`

#### Terminal 2: Run GUI
```bash
cargo run -p oxide-gui
```

#### Terminal 3: Use CLI
```bash
cargo run -p oxide-cli -- list
cargo run -p oxide-cli -- create "Test Room"
cargo run -p oxide-cli -- join <room-id>
```

### Using xtask (Recommended)

```bash
# Build portable release (debug)
cargo xtask portable

# Build portable release (optimized)
cargo xtask portable --release

# Build and ZIP
cargo xtask portable --release --zip

# Build just release binaries
cargo xtask release

# Run GUI for testing
cargo xtask dev-gui

# Run service for testing
cargo xtask dev-service

# Clean all build artifacts
cargo xtask clean
```

## Portable Build System

### Xtask Architecture

The `xtask` project provides build automation for:
1. **Portable releases**: Self-contained directories with service + GUI
2. **Release binaries**: Optimized builds with optional cross-compilation
3. **Development runs**: Quick testing without full rebuild

### Xtask Commands Reference

#### `cargo xtask portable`

Builds a portable bundle with both GUI and service.

**Options**:
- `--output <dir>` - Output directory (default: `dist`)
- `--release` - Build optimized binaries (default: debug)
- `--zip` - Create compressed archive

**Output Structure**:
```
dist/Oxide-Portable/
├── oxide           (GUI executable)
├── oxide-service   (Service daemon)
├── launch.sh       (Unix launcher script)
├── launch.bat      (Windows launcher script)
├── README.txt      (Quick start guide)
├── config/         (Config directory)
└── assets/         (UI assets)
```

**Example**:
```bash
# Create release bundle
cargo xtask portable --release --zip

# Output: dist/Oxide-Portable/ and dist/Oxide-Portable.zip
```

#### `cargo xtask release`

Builds all three binaries (oxide-gui, oxide-service, oxide-cli) in release mode.

**Options**:
- `--target <triple>` - Cross-compile target (e.g., `x86_64-pc-windows-gnu`)

**Example**:
```bash
# Native release build
cargo xtask release

# Cross-compile for another target
cargo xtask release --target aarch64-unknown-linux-gnu
```

#### `cargo xtask dev-gui` / `cargo xtask dev-service`

Quick shortcuts for development:

```bash
cargo xtask dev-gui      # Runs: cargo run -p oxide-gui
cargo xtask dev-service  # Runs: cargo run -p oxide-service
```

#### `cargo xtask clean`

Clean all build artifacts:
```bash
cargo xtask clean        # Runs: cargo clean
```

### Helper Scripts (Alternative to xtask)

If you prefer shell/batch scripts over xtask:

#### Windows
```powershell
# Build debug portable bundle
.\scripts\build-portable-windows.ps1

# Build release with ZIP
.\scripts\build-portable-windows.ps1 -Release -Zip
```

#### Linux/macOS
```bash
# Build debug portable bundle
./scripts/build-portable-unix.sh

# Build release with ZIP
./scripts/build-portable-unix.sh --release --zip
```

## Cargo.toml Configuration

### oxide-gui

```toml
eframe = { version = "0.26", features = ["wgpu", "glow"] }
```

- **Primary**: `wgpu` (Vulkan on Linux, DirectX 12 on Windows, Metal on macOS)
- **Fallback**: `glow` (OpenGL as last resort)

### oxide-service & oxide-cli

Standard workspace dependencies via `oxide-core`.

## Distribution

### For End Users

1. **GitHub Releases**: Upload `Oxide-Portable.zip` from `dist/`
2. **User downloads ZIP, extracts, and runs**:
   - Windows: Double-click `oxide.exe`
   - Linux/macOS: Run `./oxide` or `./launch.sh`

### Platform-Specific Notes

#### Windows 10/11
- Uses DirectX 12 backend via wgpu
- No OpenGL driver required
- Requires Visual C++ Runtime (usually pre-installed)
- .exe can be run directly or via PowerShell script

#### Linux
- Uses Vulkan backend via wgpu
- Requires: `libvulkan1` and graphics drivers
- AppImage distribution possible (future enhancement)
- Snap package possible (future enhancement)

#### macOS
- Uses Metal backend via wgpu
- Should work on any modern Mac with proper GPU support
- Requires Rust toolchain for building

#### WSL 2 / Docker / Headless
- **GUI will NOT work** (no graphics support)
- **CLI works fine** (use oxide-cli with running oxide-service)
- **Service works fine** (no graphics required)
- Recommendation: Run GUI on host OS, service in WSL/container

## Docker Deployment (Future)

For server-side room hosting without GUI:

```dockerfile
FROM rust:latest
WORKDIR /app
COPY . .
RUN cargo build --release -p oxide-service
CMD ["./target/release/oxide-service"]
```

```bash
docker build -t oxide-service .
docker run -d -p 8080:8080 oxide-service
```

## Troubleshooting Build Issues

### OpenGL errors (old fallback code)
- ✅ Fixed: Now uses wgpu as primary renderer
- `oxide-gui` should not fail due to missing OpenGL drivers

### Windows build fails
- Ensure Visual Studio Build Tools are installed
- Or use `rustup default stable-msvc`

### Linux missing dependencies
- wgpu requires Vulkan libraries
- Install: `sudo apt install libvulkan1 libvulkan-dev` (Ubuntu/Debian)

### macOS M1/M2 (Apple Silicon)
- Ensure Rust target is installed: `rustup target add aarch64-apple-darwin`
- Cross-compile if needed

## Performance & Optimization

### Current Status
- **Debug builds**: Suitable for development/testing
- **Release builds**: Optimized for end-user deployment
- **GUI**: egui is lightweight; performance is adequate for room management UI
- **Networking**: P2P mesh uses UDP; no overhead from GUI rendering

### Future Optimizations (Not Priority)
- Vulkan-specific optimizations
- Render graph optimization
- Memory pooling for packet buffers

## CI/CD Integration

### GitHub Actions Example (Future)

```yaml
name: Build Releases

on:
  push:
    tags: ['v*']

jobs:
  build:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo xtask portable --release --zip
      - uses: softprops/action-gh-release@v1
        with:
          files: dist/Oxide-Portable.zip
```

## FAQ

**Q: Why migrate from OpenGL to wgpu?**
A: wgpu provides modern graphics abstraction (Vulkan/DirectX/Metal) with better Windows/Linux support and fewer driver compatibility issues.

**Q: Can I use the GUI in WSL 2?**
A: Not directly (no graphics). Use oxide-cli or run GUI on host OS.

**Q: Do I need to install anything for the GUI to work?**
A: On Windows: Usually nothing (DirectX 12 built-in).
   On Linux: Vulkan libraries (usually pre-installed).
   On macOS: Should work out-of-box.

**Q: Can I run oxide-service on a server?**
A: Yes! The service has no GUI dependencies. Perfect for Docker/server deployments.

**Q: How do I update from older versions?**
A: Download new portable ZIP, extract, and run. Config is stored in OS-specific directories (not affected by binary updates).

## Next Steps

- [ ] Add AppImage distribution for Linux
- [ ] Add Snap package for Linux
- [ ] Add macOS .dmg packaging
- [ ] Add Windows installer (.msi or .exe setup)
- [ ] GitHub Actions CI/CD for automated releases
- [ ] Docker container support for server deployments
- [ ] Performance profiling and optimization
