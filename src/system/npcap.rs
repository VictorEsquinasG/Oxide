//! Npcap packet capture library management
//! 
//! Handles automatic detection and installation of Npcap,
//! a packet capture library for Windows.
//! 
//! Npcap is downloaded from official sources during installation.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::process::Command;
use crate::system::installer::SystemInstaller;

const NPCAP_MARKER_FILE: &str = "npcap_installed.marker";
const NPCAP_INSTALLER_URL: &str = "https://nmap.org/npcap/dist/npcap-1.13.exe";

fn get_marker_path() -> PathBuf {
    let app_data = std::env::var("APPDATA")
        .unwrap_or_else(|_| std::env::var("LOCALAPPDATA").unwrap_or_default());
    
    let hecate_dir = PathBuf::from(app_data).join("HecateVPN");
    let _ = std::fs::create_dir_all(&hecate_dir);
    
    hecate_dir.join(NPCAP_MARKER_FILE)
}

/// Npcap SDK installer
pub struct NpcapInstaller;

impl SystemInstaller for NpcapInstaller {
    fn name(&self) -> &str {
        "Npcap"
    }

    fn is_installed(&self) -> bool {
        is_npcap_installed()
    }

    fn install(&self, on_progress: Option<Arc<Mutex<Box<dyn Fn(String) + Send>>>>) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send>> {
        Box::pin(install_npcap(on_progress))
    }
}

/// Check if Npcap is installed and SDK is available
pub fn is_npcap_installed() -> bool {
    // On non-Windows platforms, Npcap is not needed
    #[cfg(not(windows))]
    return true;

    #[cfg(windows)]
    {
        // Method 1: Check if marker file exists from previous installation
        if get_marker_path().exists() {
            return true;
        }

        // Method 2: Check if Npcap is installed in Program Files
        let program_files = std::env::var("ProgramFiles")
            .unwrap_or_else(|_| "C:\\Program Files".to_string());
        
        let npcap_dir = PathBuf::from(&program_files).join("Npcap");
        if npcap_dir.exists() {
            // Verify DLL exists
            if npcap_dir.join("Packet.dll").exists() {
                create_marker_file();
                return true;
            }
        }

        // Method 3: Check in System32
        let dll_paths = [
            "C:\\Windows\\System32\\Packet.dll",
            "C:\\Windows\\SysWOW64\\Packet.dll",
        ];
        
        for path in &dll_paths {
            if PathBuf::from(path).exists() {
                create_marker_file();
                return true;
            }
        }

        false
    }
}

fn create_marker_file() {
    let _ = std::fs::write(get_marker_path(), b"NPCAP_INSTALLED");
}

fn get_temp_dir() -> PathBuf {
    std::env::temp_dir().join("hecate_npcap")
}

/// Download and install Npcap with progress callback
pub async fn install_npcap(on_progress: Option<Arc<Mutex<Box<dyn Fn(String) + Send>>>>) -> anyhow::Result<()> {
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

    let temp_dir = get_temp_dir();
    let _ = std::fs::create_dir_all(&temp_dir);

    // Stage 1: Download
    progress("🌐 Downloading Npcap SDK...".to_string());
    progress("  This file is small (~1-2 MB), may take a few seconds".to_string());
    
    let installer_path = temp_dir.join("npcap-1.13.exe");
    
    if !installer_path.exists() {
        match download_file(NPCAP_INSTALLER_URL, &installer_path).await {
            Ok(_) => {
                progress("✅ Download completed".to_string());
            }
            Err(e) => {
                progress(format!("❌ Download error: {}", e));
                anyhow::bail!("Failed to download Npcap: {}", e);
            }
        }
    } else {
        progress("✓ File already downloaded".to_string());
    }

    // Stage 2: Install SDK
    progress("".to_string()); // blank line
    progress("📦 Installing Npcap SDK...".to_string());
    progress("  (This process may take 1-2 minutes)".to_string());
    progress("".to_string()); // blank line

    // Run installer silently
    match Command::new(&installer_path)
        .arg("/S")  // Silent install
        .arg("/D=C:\\Npcap")  // Installation directory
        .status()
    {
        Ok(status) => {
            if !status.success() {
                progress(format!("❌ Error: Installer failed with code: {:?}", status.code()));
                anyhow::bail!("Npcap installer failed");
            }
        }
        Err(e) => {
            progress(format!("❌ Error running installer: {}", e));
            anyhow::bail!("Failed to spawn Npcap installer: {}", e);
        }
    }

    // Stage 3: Configure environment
    progress("⚙️ Configuring environment variables...".to_string());
    
    if let Err(e) = setup_npcap_environment() {
        progress(format!("⚠️ Warning configuring variables: {}", e));
        // Don't fail here, just warn
    } else {
        progress("✅ Environment variables configured".to_string());
    }

    // Success messages
    progress("".to_string()); // blank line
    progress("✅ INSTALLATION SUCCESSFUL!".to_string());
    progress("🎉 Npcap SDK is ready to use".to_string());
    progress("".to_string()); // blank line
    progress("👉 Please restart HecateVPN to complete the installation".to_string());
    progress("".to_string()); // blank line
    
    // Create marker file
    create_marker_file();
    
    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);

    Ok(())
}

async fn download_file(url: &str, path: &PathBuf) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let response = client.get(url).send().await?;
    
    if !response.status().is_success() {
        anyhow::bail!("HTTP Error: {}", response.status());
    }

    let bytes = response.bytes().await?;
    std::fs::write(path, bytes)?;
    
    Ok(())
}

fn setup_npcap_environment() -> anyhow::Result<()> {
    let npcap_dir = PathBuf::from("C:\\Npcap");
    
    if !npcap_dir.exists() {
        anyhow::bail!("Npcap directory not found");
    }

    // Set environment variable for Rust cargo build
    // NPCAP_SDK is used by build.rs if needed
    std::env::set_var("NPCAP_SDK", npcap_dir.to_str().unwrap_or(""));

    Ok(())
}
