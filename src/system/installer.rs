//! System dependency installer trait and implementations
//! 
//! Provides a unified interface for installing system dependencies like
//! Wintun and Npcap. This abstraction allows the UI layer to remain
//! independent of specific installers.

use std::sync::Arc;
use tokio::sync::Mutex;

/// Abstract interface for system dependency installers
/// 
/// Implementations handle platform-specific installation of required drivers
/// and libraries. Progress is reported via optional callback.
/// 
/// Note: Uses a boxed future for dyn compatibility, since async fn traits
/// aren't object-safe in Rust yet.
pub trait SystemInstaller: Send + Sync {
    /// Return the name of this installer (e.g., "Wintun", "Npcap")
    fn name(&self) -> &str;

    /// Check if the dependency is already installed
    fn is_installed(&self) -> bool;

    /// Install the dependency (returns a future)
    /// 
    /// # Arguments
    /// * `on_progress` - Optional callback for progress updates
    fn install(&self, on_progress: Option<Arc<Mutex<Box<dyn Fn(String) + Send>>>>) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send>>;
}

