![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white) [![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](https://www.gnu.org/licenses/gpl-3.0) 
# MINI LAN BRIDGE PORTAL
## The OpenSource Retro-Gaming Solution 


### What is it? 
* A open VPN service to create distance-LAN parties

### Designed for 
* LAN gaming over the internet.
* Access to remote Networks.

## Architecture

The project is structured as a Cargo workspace with four main components:

### Core Components

- **oxide-core**: Shared library containing networking, room management, and platform integration logic
- **oxide-service**: Persistent daemon handling room operations, networking, and IPC server
- **oxide-cli**: Command-line interface for room management and testing
- **oxide-gui**: Graphical user interface for visual room management (desktop app)

### Communication

- **GUI ↔ Service**: JSON-RPC over TCP (127.0.0.1:8080)
- **CLI ↔ Service**: Same JSON-RPC protocol
- **Service ↔ Network**: P2P UDP mesh networking (port 9000)
- **Data Storage**: JSON files in OS-specific config directories

```
┌─────────────────────────────────────────────────┐
│  oxide-gui              oxide-cli               │
│  (Visual UI)            (Commands)              │
└──────────────┬─────────────────┬────────────────┘
               │                 │
               └────────┬────────┘
                        │ IPC (JSON/TCP:8080)
                        ↓
               ┌─────────────────┐
               │ oxide-service   │
               │ (Daemon)        │
               └────────┬────────┘
                        │
         ┌──────────────┼──────────────┐
         │              │              │
         ↓              ↓              ↓
     ┌───────┐      ┌──────┐      ┌───────┐
     │ Room  │      │ P2P  │      │ Mesh  │
     │Manager│      │Network│     │Connect│
     └───────┘      └──────┘      └───────┘
         │              │              │
         └──────────────┼──────────────┘
                        │
                        ↓
              ┌──────────────────┐
              │  oxide-core      │
              │ (Shared Library) │
              └──────────────────┘
```

## GUI Rendering Backend

### Version 0.2.0+: wgpu (Modern Graphics Stack)

**Migration from OpenGL (0.1.x) → wgpu (0.2.0+)**

| Aspect | OpenGL (0.1.x) | wgpu (0.2.0+) |
|--------|----------------|--------------|
| Driver Dependency | OpenGL 2.0+ required | Modern GPU drivers (abstracted) |
| Windows Support | Fallback only | DirectX 12 primary |
| Linux Support | Basic OpenGL | Vulkan primary |
| macOS Support | OpenGL Metal bridge | Metal primary |
| WSL 2 Compatibility | ❌ No graphics | ❌ Still no graphics |
| Docker/Headless | ❌ Fails | ❌ Still not applicable |
| Distribution | Simple but brittle | Robust and modern |
| Use Case | Legacy support | Current standard |

**Why the change?**
- Easier deployment (fewer driver compatibility issues)
- Better Windows 10/11 support (DirectX 12 backend)
- Better Linux support (Vulkan instead of raw OpenGL)
- Modern graphics stack used by industry
- Graceful fallback to OpenGL if needed

**Important Notes**:
- GUI requires graphics support (native OS only, not headless/WSL/Docker)
- Service daemon works anywhere (no graphics required)
- CLI works anywhere (no graphics required)

## Quick Start

### 1. Build Portable Release

```bash
# Using xtask (recommended)
cargo xtask portable --release --zip

# Or using shell script (Windows)
.\scripts\build-portable-windows.ps1 -Release -Zip

# Or using shell script (Linux/macOS)
./scripts/build-portable-unix.sh --release --zip
```

**Output**: `dist/Oxide-Portable/` folder with:
- `oxide` / `oxide.exe` - GUI application
- `oxide-service.exe` - Background service
- `launch.sh` / `launch.bat` - Platform-specific launcher
- `config/` - Configuration directory
- `assets/` - UI assets

### 2. Deploy Portable Release

**Windows**:
```cmd
dist\Oxide-Portable\oxide.exe
```

**Linux/macOS**:
```bash
./dist/Oxide-Portable/oxide
# or
./dist/Oxide-Portable/launch.sh
```

### 3. Development Workflow

#### Terminal 1: Start Service
```bash
cargo run -p oxide-service
```

#### Terminal 2: Run GUI
```bash
cargo run -p oxide-gui
# or
cargo xtask dev-gui
```

#### Terminal 3: Use CLI
```bash
cargo run -p oxide-cli -- list
cargo run -p oxide-cli -- create "My Room"
cargo run -p oxide-cli -- join <room-id>
# or
cargo xtask dev-gui / cargo xtask dev-service
```

## Usage

### GUI Commands

Once the GUI starts:

1. **Main Menu**
   - Create Room - Host a new gaming room
   - Join Room - Connect to existing room via code
   - Legacy Mode - Direct P2P without room management

2. **In Room**
   - View players and IPs
   - Click "Connect to Network" to establish P2P connections
   - Share room code with other players

### CLI Commands

```bash
# List available rooms
cargo run -p oxide-cli -- list

# Create a new room
cargo run -p oxide-cli -- create "My Game Room"

# Join an existing room
cargo run -p oxide-cli -- join <room-id>

# Leave the current room
cargo run -p oxide-cli -- leave
```

### Service Management

The service daemon runs automatically when GUI starts. For manual control:

```bash
# Start service (persists in background)
cargo run -p oxide-service

# Service listens on 127.0.0.1:8080 for IPC commands
```

## Building from Source

### Prerequisites

- **Rust** 1.70+ ([rustup.rs](https://rustup.rs))
- **Cargo** (comes with Rust)
- **Graphics support** (for GUI only):
  - Windows: DirectX 12 (built-in on Windows 10/11)
  - Linux: Vulkan drivers (usually pre-installed)
  - macOS: Metal support (modern Macs)

### Development Build

```bash
# Build all binaries (debug mode)
cargo build

# Build specific crate
cargo build -p oxide-gui
cargo build -p oxide-service
cargo build -p oxide-cli
cargo build -p oxide-core
```

### Release Build

```bash
# Build all binaries (optimized)
cargo build --release

# Or use xtask
cargo xtask release
```

### Cross-Compilation

```bash
# Add target
rustup target add x86_64-pc-windows-gnu  # Windows from Linux
rustup target add x86_64-unknown-linux-gnu # Linux from Windows

# Build for target
cargo xtask release --target <target-triple>
```

## Documentation

- **[BUILD_GUIDE.md](BUILD_GUIDE.md)** - Detailed build and deployment guide
- **[PHASE_3_COMPLETE.md](PHASE_3_COMPLETE.md)** - P2P networking implementation (Phase 3)
- **[FINAL_REPORT_PHASE_3.md](FINAL_REPORT_PHASE_3.md)** - Phase 3 completion report
- **[TESTING_GUIDE_PHASE_3.md](TESTING_GUIDE_PHASE_3.md)** - Testing procedures
- **[QUICKSTART.md](QUICKSTART.md)** - Quick reference (Phases 1-2)

## Architecture Deep Dive

### Phase 1-2: Room Management System
- Room creation with auto-generated codes
- Player joining/leaving with virtual IP assignment
- JSON persistence across platforms
- Multi-player state synchronization

### Phase 3: P2P Mesh Networking
- Direct peer-to-peer UDP connections
- NAT traversal with STUN protocol
- Keep-alive monitoring and health checking
- Packet routing (broadcast/unicast)

### Future: Service Integration
- IPC protocol for GUI/CLI/Service
- Automatic service lifecycle management
- Portable single-executable distribution

## Platform Support

| Platform              | GUI    | Service     | CLI   | Notes |
|-----------------------|--------|-------------|-------|-------|
| Windows 10/11         |   ✅   | ✅         |   ✅  | DirectX 12 backend |
| Linux (Ubuntu/Debian) |   ✅   | ✅         | ✅    | Vulkan backend |
| macOS (Intel/M1/M2)   |   ✅   | ✅         | ✅    | Metal backend |
| WSL 2                 | ❌ GUI | ✅ Service | ✅ CLI | No graphics support |
| Docker                | ❌ GUI | ✅ Service | ✅ CLI | Headless only |

## Troubleshooting

### GUI won't start

**Windows**:
- Ensure DirectX 12 is available (Windows 10/11)
- Check GPU drivers are up-to-date
- Try running from command prompt for detailed error messages

**Linux**:
- Install Vulkan support: `sudo apt install libvulkan1 libvulkan-dev`
- Check GPU drivers support Vulkan: `vulkaninfo`
- For Intel GPUs: `sudo apt install intel-media-driver`

**macOS**:
- Ensure GPU is supported (most modern Macs are)
- Check System Report → Graphics for hardware info

### Service won't start

- Ensure port 8080 is not in use: `netstat -an | grep 8080`
- Check firewall settings allow localhost connections
- Look for error messages in terminal

### Can't connect peers

- Verify all machines can reach each other (firewall rules)
- Check NAT settings if behind firewall
- Use CLI to verify room state: `cargo run -p oxide-cli -- list`

### Performance issues

- Reduce number of peers in room (max recommended: 10)
- Check network latency between peers
- Monitor CPU/memory usage during gameplay

## License

GPL v3 - See [LICENSE](LICENSE) file

## Contributing

Contributions welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## Status

- **Phase 1-2**: ✅ Room management system complete
- **Phase 3**: ✅ P2P mesh networking complete
- **Phase 4**: Service/CLI/GUI integration (current)
- **Phase 5**: Advanced features (future)
