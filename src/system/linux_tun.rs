/// Linux TUN device implementation
/// Creates and manages a virtual TUN interface for the VPN
/// Requires root/sudo privileges to run

#[cfg(target_os = "linux")]
pub mod linux_tun {
    use super::super::tun_device::TunDeviceImpl;
    use anyhow::{Result, anyhow};
    use std::io::{Read, Write};
    use std::os::unix::io::{AsRawFd, RawFd};
    use std::process::Command;

    /// Linux TUN device wrapper
    pub struct LinuxTunDevice {
        fd: RawFd,
        ip: String,
        mtu: u16,
        name: String,
    }

    impl LinuxTunDevice {
        /// Create a new TUN device on Linux
        /// This will:
        /// 1. Create a TUN interface named 'hecate0'
        /// 2. Set the IP address
        /// 3. Enable the interface
        /// 4. Configure routing if needed
        /// 
        /// Requires: sudo/root privileges
        pub fn new(ip: &str) -> Result<Self> {
            let name = "hecate0";
            
            // Check if running as root
            if unsafe { libc::getuid() } != 0 {
                return Err(anyhow!(
                    "TUN device creation requires root privileges. Please run with: sudo {}",
                    std::env::current_exe()?.display()
                ));
            }

            // Create TUN device
            let fd = Self::create_tun_device(name)?;
            
            // Configure the interface
            Self::configure_interface(name, ip)?;
            
            Ok(LinuxTunDevice {
                fd,
                ip: ip.to_string(),
                mtu: 1500,
                name: name.to_string(),
            })
        }

        /// Create TUN device by opening /dev/net/tun
        fn create_tun_device(name: &str) -> Result<RawFd> {
            use std::ffi::CString;

            // Open /dev/net/tun
            let dev_path = CString::new("/dev/net/tun")?;
            let fd = unsafe {
                libc::open(
                    dev_path.as_ptr(),
                    libc::O_RDWR | libc::O_NONBLOCK,
                )
            };

            if fd < 0 {
                return Err(anyhow!("Failed to open /dev/net/tun: {}", std::io::Error::last_os_error()));
            }

            // Configure TUN interface
            let mut ifr = unsafe { std::mem::zeroed::<ifreq>() };
            let name_cstr = CString::new(name)?;
            
            // Copy interface name
            let name_bytes = name_cstr.as_bytes();
            if name_bytes.len() >= ifr.ifr_name.len() {
                return Err(anyhow!("Interface name too long"));
            }
            
            unsafe {
                std::ptr::copy_nonoverlapping(
                    name_cstr.as_ptr() as *const u8,
                    ifr.ifr_name.as_mut_ptr() as *mut u8,
                    name_bytes.len(),
                );
            }

            // Set TUN (not TAP) mode
            unsafe {
                ifr.ifr_ifru.ifru_flags = (libc::IFF_TUN | libc::IFF_NO_PI) as i16;
            }

            // IOCTL call to create TUN device
            let ret = unsafe {
                libc::ioctl(fd, libc::TUNSETIFF as _, &mut ifr as *mut _)
            };

            if ret < 0 {
                unsafe { libc::close(fd) };
                return Err(anyhow!("IOCTL TUNSETIFF failed: {}", std::io::Error::last_os_error()));
            }

            Ok(fd)
        }

        /// Configure the TUN interface with IP address and enable it
        fn configure_interface(name: &str, ip: &str) -> Result<()> {
            // Parse IP and calculate netmask
            let ip_parts: Vec<&str> = ip.split('/').collect();
            let (ip_addr, prefix) = if ip_parts.len() == 2 {
                (ip_parts[0], ip_parts[1].parse::<u8>().unwrap_or(24))
            } else {
                (ip, 24)
            };

            // Set IP address: ip addr add 10.0.0.1/24 dev hecate0
            let output = Command::new("ip")
                .args(&["addr", "add", &format!("{}/{}", ip_addr, prefix), "dev", name])
                .output()?;
            
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                eprintln!("Warning: Failed to set IP address: {}", stderr);
            }

            // Enable interface: ip link set hecate0 up
            let output = Command::new("ip")
                .args(&["link", "set", name, "up"])
                .output()?;
            
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow!("Failed to enable interface: {}", stderr));
            }

            // Set MTU
            let output = Command::new("ip")
                .args(&["link", "set", "mtu", "1500", "dev", name])
                .output()?;
            
            if !output.status.success() {
                eprintln!("Warning: Failed to set MTU");
            }

            Ok(())
        }

        /// Clean up the TUN device on drop
        fn cleanup(&self) {
            let _ = Command::new("ip")
                .args(&["link", "del", &self.name])
                .output();
            
            unsafe {
                libc::close(self.fd);
            }
        }
    }

    impl Drop for LinuxTunDevice {
        fn drop(&mut self) {
            self.cleanup();
        }
    }

    impl TunDeviceImpl for LinuxTunDevice {
        fn read(&mut self) -> Result<Vec<u8>> {
            let mut buf = vec![0u8; self.mtu as usize];
            
            let n = unsafe {
                libc::read(self.fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len())
            };

            if n < 0 {
                let err = std::io::Error::last_os_error();
                if err.kind() == std::io::ErrorKind::WouldBlock {
                    // No data available - not an error
                    return Ok(vec![]);
                }
                return Err(anyhow!("Failed to read from TUN: {}", err));
            }

            buf.truncate(n as usize);
            Ok(buf)
        }

        fn write(&mut self, packet: &[u8]) -> Result<()> {
            let n = unsafe {
                libc::write(self.fd, packet.as_ptr() as *const libc::c_void, packet.len())
            };

            if n < 0 {
                return Err(anyhow!("Failed to write to TUN: {}", std::io::Error::last_os_error()));
            }

            Ok(())
        }

        fn get_ip(&self) -> String {
            self.ip.clone()
        }

        fn get_mtu(&self) -> u16 {
            self.mtu
        }
    }

    // ioctl structures for TUN device creation
    #[repr(C)]
    struct ifreq {
        ifr_name: [u8; 16],
        ifr_ifru: ifr_ifru,
    }

    #[repr(C)]
    union ifr_ifru {
        ifru_flags: i16,
    }

    unsafe impl Send for LinuxTunDevice {}
    unsafe impl Sync for LinuxTunDevice {}
}

#[cfg(not(target_os = "linux"))]
pub mod linux_tun {}
