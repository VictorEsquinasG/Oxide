use anyhow::Result;
/// TUN Device abstraction for cross-platform virtual network interface
/// Factory pattern for creating platform-specific implementations
/// Supports Linux, macOS, and Windows (via wintun)
use std::sync::{Arc, Mutex, Once};

/// Virtual TUN device interface
#[derive(Clone)]
pub struct TunDevice {
    inner: Arc<TunDeviceInner>,
}

struct TunDeviceInner {
    device: Arc<Mutex<Box<dyn TunDeviceImpl + Send + Sync>>>,
}

/// Trait for platform-specific TUN implementation
/// Each platform (Linux, Windows, macOS) implements this
pub trait TunDeviceImpl {
    /// Read a packet from the TUN device
    fn read(&mut self) -> Result<Vec<u8>>;

    /// Write a packet to the TUN device
    fn write(&mut self, packet: &[u8]) -> Result<()>;

    /// Get the assigned virtual IP address
    fn get_ip(&self) -> String;

    /// Get the MTU (Maximum Transmission Unit)
    fn get_mtu(&self) -> u16;
}

static INIT: Once = Once::new();
static mut INSTANCE: Option<TunDevice> = None;

impl TunDevice {
    /// Factory: Create or get singleton TUN device
    /// Platform-aware - automatically selects correct implementation
    pub fn get_or_create(ip: String) -> Result<Self> {
        unsafe {
            INIT.call_once(|| {
                INSTANCE = Self::create_platform_device(&ip).ok();
            });

            // SAFETY: INIT.call_once ensures INSTANCE is only initialized once, 
            // and we never create a mutable reference while this shared reference exists
            #[allow(static_mut_refs)]
            match INSTANCE.as_ref() {
                Some(device) => Ok(device.clone()),
                None => Err(anyhow::anyhow!("Failed to initialize TUN device")),
            }
        }
    }

    /// Factory method: Create platform-specific device
    #[cfg(target_os = "linux")]
    fn create_platform_device(ip: &str) -> Result<Self> {
        use crate::system::linux_tun::linux_tun::LinuxTunDevice;

        let device = LinuxTunDevice::new(ip)?;
        Ok(TunDevice {
            inner: Arc::new(TunDeviceInner {
                device: Arc::new(Mutex::new(Box::new(device))),
            }),
        })
    }

    #[cfg(target_os = "windows")]
    fn create_platform_device(ip: &str) -> Result<Self> {
        use crate::system::windows_tun::windows_tun::WindowsTunDevice;

        let device = WindowsTunDevice::new(ip)?;
        Ok(TunDevice {
            inner: Arc::new(TunDeviceInner {
                device: Arc::new(Mutex::new(Box::new(device))),
            }),
        })
    }

    #[cfg(target_os = "macos")]
    fn create_platform_device(ip: &str) -> Result<Self> {
        use crate::system::macos_tun::macos_tun::MacosTunDevice;

        let device = MacosTunDevice::new(ip)?;
        Ok(TunDevice {
            inner: Arc::new(TunDeviceInner {
                device: Arc::new(Mutex::new(Box::new(device))),
            }),
        })
    }

    /// Read a packet from TUN device
    pub async fn read(&self) -> Result<Vec<u8>> {
        let mut device = self.inner.device.lock().unwrap();
        device.read()
    }

    /// Write a packet to TUN device
    pub async fn write(&self, packet: &[u8]) -> Result<()> {
        let mut device = self.inner.device.lock().unwrap();
        device.write(packet)
    }

    /// Get virtual IP address
    pub fn get_ip(&self) -> String {
        let device = self.inner.device.lock().unwrap();
        device.get_ip()
    }

    /// Get MTU
    pub fn get_mtu(&self) -> u16 {
        let device = self.inner.device.lock().unwrap();
        device.get_mtu()
    }
}
