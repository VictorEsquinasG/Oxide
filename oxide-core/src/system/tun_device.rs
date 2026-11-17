use anyhow::Result;
/// TUN Device abstraction for cross-platform virtual network interface.
/// Supports Linux (/dev/net/tun), Windows (Wintun) and macOS (stub).
use std::sync::Arc;

/// Platform-specific TUN implementation.
///
/// All methods take `&self` so one thread can block in `read` while another
/// calls `write`. Packets are bare L3 IPv4 packets (no Ethernet header).
pub trait TunDeviceImpl: Send + Sync {
    /// Block until a packet arrives or ~200 ms pass. Returns an empty vec on timeout.
    fn read(&self) -> Result<Vec<u8>>;

    /// Write a packet to the TUN device
    fn write(&self, packet: &[u8]) -> Result<()>;

    /// Get the assigned virtual IP address
    fn get_ip(&self) -> String;

    /// Get the MTU (Maximum Transmission Unit)
    fn get_mtu(&self) -> u16;
}

/// Virtual TUN device handle (cheap to clone)
#[derive(Clone)]
pub struct TunDevice {
    inner: Arc<dyn TunDeviceImpl>,
}

impl TunDevice {
    /// Create the platform TUN device with the given virtual IP ("10.0.0.2" or "10.0.0.2/24").
    pub fn new(ip: &str) -> Result<Self> {
        Self::create_platform_device(ip)
    }

    #[cfg(target_os = "linux")]
    fn create_platform_device(ip: &str) -> Result<Self> {
        use crate::system::linux_tun::linux_tun::LinuxTunDevice;
        Ok(TunDevice {
            inner: Arc::new(LinuxTunDevice::new(ip)?),
        })
    }

    #[cfg(target_os = "windows")]
    fn create_platform_device(ip: &str) -> Result<Self> {
        use crate::system::windows_tun::windows_tun::WindowsTunDevice;
        Ok(TunDevice {
            inner: Arc::new(WindowsTunDevice::new(ip)?),
        })
    }

    #[cfg(target_os = "macos")]
    fn create_platform_device(ip: &str) -> Result<Self> {
        use crate::system::macos_tun::macos_tun::MacosTunDevice;
        Ok(TunDevice {
            inner: Arc::new(MacosTunDevice::new(ip)?),
        })
    }

    /// Blocking read. Call from a dedicated thread (see `VpnTunnel`).
    pub fn read_blocking(&self) -> Result<Vec<u8>> {
        self.inner.read()
    }

    /// Write a packet to TUN device
    pub fn write_blocking(&self, packet: &[u8]) -> Result<()> {
        self.inner.write(packet)
    }

    /// Get virtual IP address
    pub fn get_ip(&self) -> String {
        self.inner.get_ip()
    }

    /// Get MTU
    pub fn get_mtu(&self) -> u16 {
        self.inner.get_mtu()
    }
}
