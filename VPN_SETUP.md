# HecateVPN - LAN Emulation over Internet

A lightweight, cross-platform VPN designed to emulate LAN connectivity for legacy games and applications that require local network play.

## Features

✨ **Plug & Play**: No complex configuration needed
🔗 **Peer-to-Peer**: Direct connection between two machines
🎮 **Game Ready**: Designed specifically for LAN games
🐧 **Linux Support**: Fully functional TUN virtual interface
🪟 **Windows Support**: Wintun integration (in progress)
🍎 **macOS Support**: utun support (planned)
🔒 **Encrypted**: UDP-based P2P connection
🌍 **NAT Traversal**: Automatically detects and handles NAT reflexive addresses

## Installation

### Linux

```bash
# Clone repository
git clone https://github.com/yourusername/HecateVPN
cd HecateVPN

# Build release binary (requires Rust)
cargo build --release

# Run with sudo (required for TUN device creation)
sudo ./target/release/HecateVPN
```

**Requirements:**
- Linux kernel 3.10+ (most modern systems)
- Root/sudo privileges for TUN device creation
- `ip` command available (usually pre-installed)

### Windows

```bash
cargo build --release
.\target\release\HecateVPN.exe
```

**Requirements:**
- Windows 7 or later
- Administrator privileges (for Wintun driver)

### macOS

```bash
cargo build --release
./target/release/HecateVPN
```

**Requirements:**
- macOS 10.7+ (for utun support)

## Usage

1. **Start HecateVPN on both machines:**
   ```bash
   sudo ./target/release/HecateVPN
   ```

2. **One machine acts as the server** (opens the listening port)
   - The IP and port will be displayed in the UI
   - Share this information with your peer

3. **The other machine connects** 
   - Enter the peer's IP and port in the UI
   - Click "Connect"
   - The VPN will automatically establish the connection

4. **Once connected:**
   - Your application will see a virtual LAN interface (hecate0)
   - IP range: 10.0.0.0/24
   - Both machines get IPs in this range
   - Games/apps can connect via LAN as if on the same network

## Architecture

### Core Components

**NetworkNode**: Handles UDP P2P communication
- HELLO/HELLO_ACK handshake
- PING/PONG keep-alive
- NAT reflexive address detection
- Automatic peer address update

**TunDevice** (Factory Pattern): Platform-specific virtual interface
- **Linux**: TUN device via /dev/net/tun
- **Windows**: Wintun driver integration
- **macOS**: utun socket interface

**VpnTunnel**: Bridges TUN device and UDP network
- Reads application packets from TUN
- Sends them to remote peer via UDP
- Receives remote packets via UDP
- Writes them back to TUN

**PacketHandler**: ARP and IPv4 packet manipulation
- Handles ARP requests automatically
- Packet parsing and manipulation
- IPv4 source/destination extraction

### Design Principles

- **KISS Principle**: Single return per function, early exits
- **Factory Pattern**: Platform-specific implementations
- **Singleton Pattern**: Single TUN device instance
- **Zero Configuration**: Plug & play operation

## TODO: Planned Features

- [ ] Support for >2 peer connections (multicast ARP)
- [ ] Windows Wintun full implementation
- [ ] macOS utun full implementation
- [ ] IP address auto-negotiation
- [ ] Encryption layer (optional)
- [ ] Web UI dashboard
- [ ] Docker containerization

## Troubleshooting

### Linux

**Error: "TUN device creation requires root privileges"**
```bash
# Run with sudo
sudo ./target/release/HecateVPN
```

**Interface not appearing**
```bash
# Check if interface was created
ip link show hecate0

# Check IP assignment
ip addr show hecate0

# Check routes
ip route | grep hecate0
```

**No connectivity**
```bash
# Verify both peers are connected
# Check logs in the UI for connection status
# Ensure firewall allows UDP port 9000
```

### Windows

**Driver installation issues**
- The app will automatically download and install Wintun
- If stuck, run as Administrator
- Check antivirus isn't blocking the driver

### macOS

**Permission denied**
- macOS may require additional privileges
- Try running from Terminal with: `sudo ./target/release/HecateVPN`

## Development

### Build for Linux

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Run tests
cargo test
```

### Code Structure

```
src/
├── main.rs           # Application entry point
├── app.rs            # Global application state
├── ui/               # UI components (egui)
├── network/
│   ├── node.rs       # P2P UDP network layer
│   ├── packet_handler.rs  # Packet manipulation
│   └── vpn_tunnel.rs # TUN-Network bridge
├── system/
│   ├── tun_device.rs # Factory pattern for TUN
│   ├── linux_tun.rs  # Linux implementation
│   ├── windows_tun.rs # Windows implementation
│   └── macos_tun.rs  # macOS implementation
├── packet.rs         # Packet structures
└── config.rs         # Configuration management
```

## Performance Notes

- Uses non-blocking I/O for all operations
- Minimal CPU overhead in idle state
- Supports MTU up to 1500 bytes (configurable)
- ARP caching to reduce overhead

## Security Considerations

- No built-in encryption (relies on UDP which is not encrypted)
- Designed for LAN emulation, not as a security tool
- Use only on trusted networks
- For production use, add encryption layer

## Contributing

Issues and pull requests are welcome!

## License

[Your License Here]

## Support

For issues and questions:
- Open a GitHub issue
- Check existing documentation
- Review logs in the application UI

---

**Note:** This project is designed for educational purposes and legacy game compatibility. Use responsibly!
