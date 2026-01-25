//! Configuration management module
//! 
//! Currently unused - configuration is handled entirely through the GUI.
//! Reserved for future implementation of file-based or CLI configuration.

use std::net::SocketAddr;

/// Configuration placeholder for future use
/// 
/// Intended for future support of:
/// - Configuration files (TOML/JSON)
/// - Environment variables
/// - CLI arguments
/// 
/// Currently all configuration comes from the UI layer.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Config {
    /// Local address to bind the UDP socket
    pub bind_addr: SocketAddr,
    /// Remote peer address
    pub peer_addr: SocketAddr,
}

impl Config {
    /// Parse configuration from command-line arguments (future use)
    /// 
    /// Currently unused but reserved for CLI support.
    #[allow(dead_code)]
    pub fn from_args() -> Self {
        let args: Vec<String> = std::env::args().collect();

        if args.len() != 3 {
            eprintln!("Usage:");
            eprintln!("  hecate-vpn <bind_addr> <peer_addr>");
            std::process::exit(1);
        }

        let bind_addr: SocketAddr = args[1]
            .parse()
            .expect("Invalid bind address");

        let peer_addr: SocketAddr = args[2]
            .parse()
            .expect("Invalid peer address");

        Self {
            bind_addr,
            peer_addr,
        }
    }
}

/*
TODO:
- Support config file (config.toml)
- Support GUI input
- Validate ports and IP ranges
*/
