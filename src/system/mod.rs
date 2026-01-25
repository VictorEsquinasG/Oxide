//! System-level module - Platform-specific device and driver management
//! 
//! Handles TUN device creation, system installers for dependencies,
//! and platform-specific networking implementations.

pub mod installer;
pub mod wintun;
pub mod npcap;
pub mod tun_device;
pub mod linux_tun;
pub mod windows_tun;
pub mod macos_tun;

// Re-export installer trait and concrete implementations
pub use installer::SystemInstaller;
pub use wintun::WintunInstaller;
pub use npcap::NpcapInstaller;
