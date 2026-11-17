/// Windows TUN device implementation using Wintun (https://www.wintun.net).
/// `wintun.dll` is embedded in the binary and extracted next to the executable
/// on first run. Requires Administrator privileges to create the adapter.

#[cfg(target_os = "windows")]
pub mod windows_tun {
    use super::super::tun_device::TunDeviceImpl;
    use anyhow::{anyhow, Context, Result};
    use std::net::Ipv4Addr;
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    use std::sync::Arc;

    const WINTUN_DLL: &[u8] = include_bytes!("../../../dependences/wintun.dll");
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const ADAPTER_NAME: &str = "Oxide";

    pub struct WindowsTunDevice {
        session: Arc<wintun::Session>,
        // Keep the adapter alive for as long as the session is used
        _adapter: Arc<wintun::Adapter>,
        ip: String,
        mtu: u16,
    }

    fn ensure_dll() -> Result<std::path::PathBuf> {
        let dir = std::env::current_exe()?
            .parent()
            .ok_or_else(|| anyhow!("no exe dir"))?
            .to_path_buf();
        let path = dir.join("wintun.dll");
        if std::fs::read(&path).map(|b| b != WINTUN_DLL).unwrap_or(true) {
            std::fs::write(&path, WINTUN_DLL)
                .with_context(|| format!("cannot write {}", path.display()))?;
        }
        Ok(path)
    }

    fn netsh(args: &[&str]) -> Result<()> {
        let out = Command::new("netsh")
            .args(args)
            .creation_flags(CREATE_NO_WINDOW)
            .output()?;
        if !out.status.success() {
            return Err(anyhow!(
                "netsh {:?} failed: {}{}",
                args,
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        Ok(())
    }

    impl WindowsTunDevice {
        pub fn new(ip: &str) -> Result<Self> {
            let addr_str = ip.split('/').next().unwrap_or(ip);
            let addr: Ipv4Addr = addr_str.parse().context("invalid virtual IP")?;

            let dll = ensure_dll()?;
            let lib = unsafe { wintun::load_from_path(&dll) }
                .map_err(|e| anyhow!("cannot load wintun.dll (run as Administrator): {e}"))?;
            let adapter = wintun::Adapter::create(&lib, ADAPTER_NAME, "Oxide", None)
                .map_err(|e| anyhow!("cannot create Wintun adapter (run as Administrator): {e}"))?;
            let session = Arc::new(
                adapter
                    .start_session(wintun::MAX_RING_CAPACITY)
                    .map_err(|e| anyhow!("cannot start Wintun session: {e}"))?,
            );

            // 10.0.0.x/24, no gateway. Low interface metric so 255.255.255.255
            // broadcasts from games prefer this adapter.
            netsh(&[
                "interface",
                "ip",
                "set",
                "address",
                &format!("name={}", ADAPTER_NAME),
                "static",
                &addr.to_string(),
                "255.255.255.0",
            ])?;
            let _ = netsh(&[
                "interface",
                "ip",
                "set",
                "interface",
                ADAPTER_NAME,
                "metric=1",
            ]);

            let _ = netsh(&[
                "interface",
                "ipv4",
                "set",
                "subinterface",
                ADAPTER_NAME,
                "mtu=1400",
                "store=active",
            ]);

            Ok(WindowsTunDevice {
                session,
                _adapter: adapter,
                ip: addr.to_string(),
                mtu: 1400,
            })
        }
    }

    impl Drop for WindowsTunDevice {
        fn drop(&mut self) {
            // Unblocks any thread waiting in receive_blocking
            let _ = self.session.shutdown();
        }
    }

    impl TunDeviceImpl for WindowsTunDevice {
        fn read(&self) -> Result<Vec<u8>> {
            // Blocks until a packet arrives; returns Err once the session is shut down.
            match self.session.receive_blocking() {
                Ok(pkt) => Ok(pkt.bytes().to_vec()),
                Err(e) => Err(anyhow!("Wintun read failed: {e}")),
            }
        }

        fn write(&self, packet: &[u8]) -> Result<()> {
            let mut out = self
                .session
                .allocate_send_packet(packet.len() as u16)
                .map_err(|e| anyhow!("Wintun alloc failed: {e}"))?;
            out.bytes_mut().copy_from_slice(packet);
            self.session.send_packet(out);
            Ok(())
        }

        fn get_ip(&self) -> String {
            self.ip.clone()
        }

        fn get_mtu(&self) -> u16 {
            self.mtu
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub mod windows_tun {}
