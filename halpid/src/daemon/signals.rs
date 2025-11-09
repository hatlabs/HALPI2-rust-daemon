//! Signal handling for graceful shutdown

use std::path::Path;
use tracing::info;

use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::warn;

#[cfg(unix)]
use tokio::signal::unix::{SignalKind, signal};

use crate::i2c::HalpiDevice;

/// Wait for SIGINT or SIGTERM signal
pub async fn wait_for_signal() {
    #[cfg(unix)]
    {
        let mut sigint =
            signal(SignalKind::interrupt()).expect("Failed to register SIGINT handler");
        let mut sigterm =
            signal(SignalKind::terminate()).expect("Failed to register SIGTERM handler");

        tokio::select! {
            _ = sigint.recv() => {
                info!("Received SIGINT");
            }
            _ = sigterm.recv() => {
                info!("Received SIGTERM");
            }
        }
    }

    #[cfg(not(unix))]
    {
        // On non-Unix platforms, wait for Ctrl+C
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for Ctrl+C");
        info!("Received Ctrl+C");
    }
}

/// Cleanup function to run before shutdown
///
/// This function:
/// - Disables the hardware watchdog (critical for safety)
/// - Removes the Unix socket file
/// - Flushes logs
pub async fn cleanup(device: Arc<Mutex<HalpiDevice>>, socket_path: &Path) {
    info!("Running cleanup before shutdown");

    // Disable watchdog - CRITICAL for hardware safety
    {
        let mut dev = device.lock().await;
        if let Err(e) = dev.set_watchdog_timeout(0) {
            warn!("Failed to disable watchdog during shutdown: {}", e);
        } else {
            info!("Watchdog disabled");
        }
    }

    // Remove Unix socket file
    if socket_path.exists() {
        if let Err(e) = std::fs::remove_file(socket_path) {
            warn!("Failed to remove socket file: {}", e);
        } else {
            info!("Removed socket file");
        }
    }

    // Flush logs (tracing handles this automatically on drop)
    info!("Cleanup complete");
}
