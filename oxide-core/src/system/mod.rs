//! System-level module - Platform-specific device and driver management
//!
//! Handles TUN device creation, system installers for dependencies,
//! and platform-specific networking implementations.

pub mod installer;
pub mod linux_tun;
pub mod macos_tun;
pub mod npcap;
pub mod tun_device;
pub mod windows_tun;
pub mod wintun;

// Re-export installer trait and concrete implementations
#[allow(unused_imports)]
pub use installer::SystemInstaller;
#[allow(unused_imports)]
pub use npcap::NpcapInstaller;
#[allow(unused_imports)]
pub use wintun::WintunInstaller;
