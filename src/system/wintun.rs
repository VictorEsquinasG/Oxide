//! Wintun driver management
//! 
//! Handles automatic detection and installation of Wintun,
//! a userspace TUN driver for Windows.
//! 
//! Wintun is embedded in dependences/wireguard-installer.exe

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::process::{Command, Stdio};
use std::time::Duration;
use crate::system::installer::SystemInstaller;

const WG_INSTALLER: &[u8] = include_bytes!("../../dependences/wireguard-installer.exe");
const WINTUN_MARKER_FILE: &str = "wintun_installed.marker";

fn get_marker_path() -> PathBuf {
    let app_data = std::env::var("APPDATA")
        .unwrap_or_else(|_| std::env::var("LOCALAPPDATA").unwrap_or_default());
    
    let hecate_dir = PathBuf::from(app_data).join("Oxide");
    let _ = std::fs::create_dir_all(&hecate_dir);
    
    hecate_dir.join(WINTUN_MARKER_FILE)
}

/// Wintun driver installer
pub struct WintunInstaller;

impl SystemInstaller for WintunInstaller {
    fn name(&self) -> &str {
        "Wintun"
    }

    fn is_installed(&self) -> bool {
        is_wintun_installed()
    }

    fn install(&self, on_progress: Option<Arc<Mutex<Box<dyn Fn(String) + Send>>>>) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send>> {
        Box::pin(install_wintun(on_progress))
    }
}

/// Check if wintun.dll is installed and accessible
pub fn is_wintun_installed() -> bool {
    // On non-Windows platforms, Wintun is not needed
    #[cfg(not(windows))]
    return true;

    #[cfg(windows)]
    {
        // Method 1: Check if marker file exists from previous installation
        if get_marker_path().exists() {
            return true;
        }

        // Method 2: Check for DLL in standard system paths
        let dll_paths = [
            "C:\\Windows\\System32\\wintun.dll",
            "C:\\Windows\\SysWOW64\\wintun.dll",
        ];
        
        for path in &dll_paths {
            if PathBuf::from(path).exists() {
                create_marker_file();
                return true;
            }
        }

        // Method 3: Check via pnputil (less reliable but attempt anyway)
        let result = Command::new("pnputil")
            .arg("/enum-drivers")
            .output()
            .map(|o| {
                let text = String::from_utf8_lossy(&o.stdout);
                text.contains("Wintun") || text.contains("wintun")
            })
            .unwrap_or(false);

        if result {
            create_marker_file();
        }

        result
    }
}

fn create_marker_file() {
    let _ = std::fs::write(get_marker_path(), b"WINTUN_INSTALLED");
}

fn extract_wireguard_installer() -> std::path::PathBuf {
    let path = std::env::temp_dir().join("wg_installer.exe");
    std::fs::write(&path, WG_INSTALLER).unwrap();
    path
}

/// Download and install Wintun with progress callback
pub async fn install_wintun(on_progress: Option<Arc<Mutex<Box<dyn Fn(String) + Send>>>>) -> anyhow::Result<()> {
    let progress = |msg: String| {
        if let Some(ref callback) = on_progress {
            let callback = callback.clone();
            let _ = std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new();
                if let Ok(rt) = rt {
                    rt.block_on(async {
                        callback.lock().await(msg);
                    });
                }
            });
        }
    };

    // Progress bar stages
    let stages = vec![
        ("📦 Extracting installer...", 20),
        ("⏳ Preparing installer...", 40),
        ("⚙️ Running WireGuard installer...", 60),
        ("🔄 Configuring tunnel service...", 80),
    ];

    for (stage, percent) in &stages {
        progress(format!("{} [{}%]", stage, percent));
        
        // Create progress bar
        let bar_length = 30;
        let filled = (bar_length * percent) / 100;
        let _empty = bar_length - filled;
        let bar = format!(
            "  [{:<width$}] {}%",
            "=".repeat(filled),
            percent,
            width = bar_length
        );
        progress(bar);
        
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    progress("📦 Extracting installer...".to_string());
    
    let installer = extract_wireguard_installer();
    
    progress("⏳ Running WireGuard installer...".to_string());
    progress("(This process may take 1-2 minutes)".to_string());
    progress("".to_string()); // blank line

    let mut child = Command::new(&installer)
        .arg("/installtunnelservice")
        .arg("/quiet")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    // Wait with a timeout of 5 minutes
    let timeout = Duration::from_secs(300);
    let start = std::time::Instant::now();
    
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    progress("❌ Error: WireGuard installer failed".to_string());
                    anyhow::bail!("WireGuard installer failed with exit code: {:?}", status.code());
                }
                break;
            }
            Ok(None) => {
                // Still running
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    progress("❌ Error: Timeout waiting for installer (5 minutes)".to_string());
                    anyhow::bail!("WireGuard installer timeout");
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            Err(e) => {
                progress(format!("❌ Error running installer: {}", e));
                anyhow::bail!("Failed to spawn installer: {}", e);
            }
        }
    }

    // Success messages with progress
    progress("".to_string()); // blank line
    progress("✅ INSTALLATION SUCCESSFUL!".to_string());
    progress("🎉 Wintun is ready to use".to_string());
    progress("".to_string()); // blank line
    progress("👉 Please close this window and restart Oxide".to_string());
    progress("".to_string()); // blank line
    
    // Create marker file to prevent reinstallation
    create_marker_file();
    
    // Cleanup
    let _ = std::fs::remove_file(&installer);

    Ok(())
}


