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

use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use crate::i2c::device::HalpiDevice;

/// Run the LED socket server
///
/// Listens for connections on the given socket path. Only one client can be
/// connected at a time (first-client locking). Each message is a length-prefixed
/// binary frame forwarded to the firmware via I2C register 0x60.
pub async fn run_led_socket(
    device: Arc<Mutex<HalpiDevice>>,
    num_leds: usize,
    socket_path: PathBuf,
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

    // Set socket permissions
    setup_led_socket_permissions(&socket_path, "halpid")?;

    info!("LED socket listening on {}", socket_path.display());

    let expected_payload_len = num_leds * 6;
    let active_client: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));

    loop {
        let (stream, _addr) = listener.accept().await?;

        // Check if a client is already connected
        {
            let mut active = active_client.lock().await;
            if *active {
                // Reject: another client is connected
                drop(stream);
                continue;
            }
            *active = true;
        }

        let device = device.clone();
        let active_client = active_client.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_led_client(stream, device, expected_payload_len).await {
                warn!("LED client disconnected: {}", e);
            } else {
                info!("LED client disconnected");
            }
            // Release the lock
            *active_client.lock().await = false;
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
            warn!(
                "LED socket: invalid length {}, expected {}. Closing connection.",
                length, expected_len
            );
            return Ok(());
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

/// Set socket permissions (matching HTTP socket pattern)
fn setup_led_socket_permissions(socket_path: &Path, group_name: &str) -> anyhow::Result<()> {
    use std::ffi::CString;
    use std::os::unix::fs::PermissionsExt;

    // Set permissions to 0660
    let permissions = std::fs::Permissions::from_mode(0o660);
    std::fs::set_permissions(socket_path, permissions)?;

    // Set group ownership
    let group_name_c = CString::new(group_name)?;
    let grp = unsafe { libc::getgrnam(group_name_c.as_ptr()) };
    if !grp.is_null() {
        let gid = unsafe { (*grp).gr_gid };
        let uid = unsafe { libc::getuid() };
        let path_c = CString::new(socket_path.to_str().unwrap_or(""))?;
        unsafe {
            libc::chown(path_c.as_ptr(), uid, gid);
        }
    }

    Ok(())
}
