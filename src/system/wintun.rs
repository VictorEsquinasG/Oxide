use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use std::process::{Command, Stdio};
use std::time::Duration;

const WG_INSTALLER: &[u8] = include_bytes!("../../dependences/wireguard-installer.exe");
const WINTUN_MARKER_FILE: &str = "wintun_installed.marker";

fn get_marker_path() -> PathBuf {
    let app_data = std::env::var("APPDATA")
        .unwrap_or_else(|_| std::env::var("LOCALAPPDATA").unwrap_or_default());
    
    let hecate_dir = PathBuf::from(app_data).join("HecateVPN");
    let _ = std::fs::create_dir_all(&hecate_dir);
    
    hecate_dir.join(WINTUN_MARKER_FILE)
}

/// Check if wintun.dll is installed and accessible
pub fn is_wintun_installed() -> bool {
    // Método 1: Verificar si el archivo marker existe (instalación anterior)
    if get_marker_path().exists() {
        return true;
    }

    // Método 2: Verificar la DLL en rutas estándar
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

    // Método 3: Verificar mediante pnputil (menos confiable pero intenta)
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
        ("📦 Extrayendo instalador...", 20),
        ("⏳ Preparando instalador...", 40),
        ("⚙️ Ejecutando instalador de WireGuard...", 60),
        ("🔄 Configurando servicio de túnel...", 80),
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

    progress("📦 Extrayendo instalador...".to_string());
    
    let installer = extract_wireguard_installer();
    
    progress("⏳ Ejecutando instalador de WireGuard...".to_string());
    progress("(Este proceso puede tomar 1-2 minutos)".to_string());
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
                    progress("❌ Error: Instalador de WireGuard falló".to_string());
                    anyhow::bail!("WireGuard installer failed with exit code: {:?}", status.code());
                }
                break;
            }
            Ok(None) => {
                // Still running
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    progress("❌ Error: Timeout esperando al instalador (5 minutos)".to_string());
                    anyhow::bail!("WireGuard installer timeout");
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            Err(e) => {
                progress(format!("❌ Error ejecutando instalador: {}", e));
                anyhow::bail!("Failed to spawn installer: {}", e);
            }
        }
    }

    // Success messages with progress
    progress("".to_string()); // blank line
    progress("╔════════════════════════════════════════╗".to_string());
    progress("║  ✅ ¡INSTALACIÓN EXITOSA!            ║".to_string());
    progress("║  🎉 Wintun está listo para usar       ║".to_string());
    progress("╚════════════════════════════════════════╝".to_string());
    progress("".to_string()); // blank line
    progress("👉 Por favor, cierra esta ventana y reinicia HecateVPN".to_string());
    progress("".to_string()); // blank line
    
    // Create marker file to prevent reinstallation
    create_marker_file();
    
    // Cleanup
    let _ = std::fs::remove_file(&installer);

    Ok(())
}
