//! Signal handling for graceful shutdown

use std::path::Path;
use tracing::info;

#[cfg(target_os = "linux")]
use std::sync::Arc;
#[cfg(target_os = "linux")]
use tokio::sync::Mutex;
#[cfg(target_os = "linux")]
use tracing::warn;

#[cfg(unix)]
use tokio::signal::unix::{SignalKind, signal};

#[cfg(target_os = "linux")]
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
#[cfg(target_os = "linux")]
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

/// Stub cleanup for non-Linux platforms
#[cfg(not(target_os = "linux"))]
pub async fn cleanup(_socket_path: &Path) {
    info!("Running cleanup before shutdown");
    // Just remove socket if it exists
    if _socket_path.exists() {
        let _ = std::fs::remove_file(_socket_path);
    }
    info!("Cleanup complete");
}
