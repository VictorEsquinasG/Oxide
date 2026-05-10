use std::path::PathBuf;

fn main() {
    // Configure Npcap SDK path for linking on Windows only
    #[cfg(target_os = "windows")]
    {
        // Try to find Npcap in common locations
        let npcap_paths = vec![
            "C:\\Npcap",
            "C:\\Program Files\\Npcap",
            "C:\\npcap-sdk",
            "C:\\npcap-1.13",
        ];

        let mut found = false;

        // Attempt to find Npcap installation
        for path in &npcap_paths {
            let packet_lib_x64 = PathBuf::from(path)
                .join("Lib")
                .join("x64")
                .join("Packet.lib");

            if packet_lib_x64.exists() {
                let lib_dir = PathBuf::from(path).join("Lib").join("x64");
                println!("cargo:rustc-link-search=native={}", lib_dir.display());
                println!("cargo:rustc-link-lib=Packet");
                println!("cargo:warning=✓ Npcap SDK found at {}", path);
                found = true;
                break;
            }

            // Also check Lib without x64 subdirectory
            let packet_lib = PathBuf::from(path).join("Lib").join("Packet.lib");

            if packet_lib.exists() {
                let lib_dir = PathBuf::from(path).join("Lib");
                println!("cargo:rustc-link-search=native={}", lib_dir.display());
                println!("cargo:rustc-link-lib=Packet");
                println!("cargo:warning=✓ Npcap SDK found at {}", path);
                found = true;
                break;
            }
        }

        if !found {
            println!("cargo:warning=⚠ Npcap SDK not found in standard locations");
            println!(
                "cargo:warning=The application will attempt to install it automatically at runtime"
            );
            println!("cargo:warning=Packet functions will be dynamically loaded - no static linking required");
        }
    }

    // No action needed for non-Windows platforms
    #[cfg(not(target_os = "windows"))]
    {}
}
