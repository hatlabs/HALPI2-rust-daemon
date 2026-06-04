//! Binary stream socket for LED override control
//!
//! Provides a Unix stream socket at `/run/halpid/led.sock` for high-frequency
//! LED override updates. The daemon acts as a pure relay: it reads length-prefixed
//! binary messages from the socket and forwards them to the firmware via I2C.
//!
//! Protocol: `[Length(1), Payload(Length bytes)]`
//! - Length must equal `num_leds * 6`
//! - Payload: per LED `[R, G, B, Alpha, TransitionMs(BE)]`
//!
//! Connection locking: first connected client prevails. New connections are
//! immediately closed while a client is active.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use super::app::setup_socket_permissions;
use crate::i2c::device::HalpiDevice;

/// Guard that releases the active_client flag on drop, even if the task panics.
struct ActiveClientGuard {
    flag: Arc<Mutex<bool>>,
}

impl Drop for ActiveClientGuard {
    fn drop(&mut self) {
        // Use try_lock to avoid blocking in drop. If lock is contended,
        // spawn a task to release it asynchronously.
        if let Ok(mut active) = self.flag.try_lock() {
            *active = false;
        } else {
            let flag = self.flag.clone();
            tokio::spawn(async move {
                *flag.lock().await = false;
            });
        }
    }
}

/// Run the LED socket server
///
/// Listens for connections on the given socket path. Only one client can be
/// connected at a time (first-client locking). Each message is a length-prefixed
/// binary frame forwarded to the firmware via I2C register 0x60.
pub async fn run_led_socket(
    device: Arc<Mutex<HalpiDevice>>,
    num_leds: usize,
    socket_path: PathBuf,
    socket_gid: u32,
) -> anyhow::Result<()> {
    // Remove existing socket if it exists
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }

    // Create parent directory if needed
    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let listener = UnixListener::bind(&socket_path)?;

    // Reuse shared socket permission setup from app.rs
    setup_socket_permissions(&socket_path, socket_gid).await?;

    info!("LED socket listening on {}", socket_path.display());

    let expected_payload_len = num_leds * 6;
    let active_client: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));

    loop {
        let (stream, _addr) = listener.accept().await?;

        // Check if a client is already connected
        {
            let mut active = active_client.lock().await;
            if *active {
                drop(stream);
                continue;
            }
            *active = true;
        }

        let device = device.clone();
        let active_client = active_client.clone();

        tokio::spawn(async move {
            // Guard ensures flag is released even on panic
            let _guard = ActiveClientGuard {
                flag: active_client,
            };

            if let Err(e) = handle_led_client(stream, device, expected_payload_len).await {
                warn!("LED client disconnected: {}", e);
            } else {
                info!("LED client disconnected");
            }
        });
    }
}

/// Handle a single LED client connection
async fn handle_led_client(
    mut stream: UnixStream,
    device: Arc<Mutex<HalpiDevice>>,
    expected_len: usize,
) -> anyhow::Result<()> {
    info!("LED client connected");

    let mut payload = vec![0u8; expected_len];

    loop {
        // Read 1-byte length prefix
        let length = stream.read_u8().await?;

        if length as usize != expected_len {
            return Err(anyhow::anyhow!(
                "invalid LED payload length: {} (expected {})",
                length,
                expected_len
            ));
        }

        // Read payload
        stream.read_exact(&mut payload).await?;

        // Forward to firmware via I2C
        let mut dev = device.lock().await;
        if let Err(e) = dev.set_led_overrides(&payload) {
            error!("LED override I2C write failed: {}", e);
        }
    }
}
