//! Small persistent settings for the VPN: client identity, last host, hub leases.

use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::path::PathBuf;

/// Per-OS config directory (same place the room manager uses).
pub fn config_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        PathBuf::from(std::env::var("APPDATA").unwrap_or_else(|_| ".".into())).join("Oxide")
    }
    #[cfg(target_os = "macos")]
    {
        PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()))
            .join("Library/Application Support/Oxide")
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into())).join(".config/Oxide")
    }
}

fn read(name: &str) -> Option<String> {
    std::fs::read_to_string(config_dir().join(name))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn write(name: &str, content: &str) {
    let dir = config_dir();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join(name), content);
}

fn hex(id: &[u8; 8]) -> String {
    id.iter().map(|b| format!("{:02x}", b)).collect()
}

fn unhex(s: &str) -> Option<[u8; 8]> {
    if s.len() != 16 {
        return None;
    }
    let mut out = [0u8; 8];
    for (i, o) in out.iter_mut().enumerate() {
        *o = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// Stable random id for this installation, so the hub gives us the same IP every time.
pub fn client_id() -> [u8; 8] {
    if let Some(id) = read("client_id").and_then(|s| unhex(&s)) {
        return id;
    }
    let id: [u8; 8] = rand::random();
    write("client_id", &hex(&id));
    id
}

/// Host address to prefill: last used, else `host.txt` next to the executable.
pub fn load_host() -> String {
    if let Some(h) = read("last_host.txt") {
        return h;
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("host.txt")))
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

pub fn save_host(host: &str) {
    write("last_host.txt", host.trim());
}

/// Saved shared key (empty if none).
pub fn load_key() -> String {
    read("key.txt").unwrap_or_default()
}

pub fn save_key(key: &str) {
    write("key.txt", key.trim());
}

/// Hub leases: client id -> virtual IP.
pub fn load_leases() -> HashMap<[u8; 8], Ipv4Addr> {
    let mut map = HashMap::new();
    if let Some(text) = read("leases.txt") {
        for line in text.lines() {
            if let Some((id, ip)) = line.split_once(' ') {
                if let (Some(id), Ok(ip)) = (unhex(id), ip.trim().parse()) {
                    map.insert(id, ip);
                }
            }
        }
    }
    map
}

pub fn save_leases(map: &HashMap<[u8; 8], Ipv4Addr>) {
    let text: String = map
        .iter()
        .map(|(id, ip)| format!("{} {}\n", hex(id), ip))
        .collect();
    write("leases.txt", &text);
}
