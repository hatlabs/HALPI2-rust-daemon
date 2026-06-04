//! Axum application setup and shared state

use axum::Router;
use halpi_common::config::Config;
use halpi_common::error::{AppError, ServerError};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tower_http::trace::TraceLayer;

use crate::i2c::device::HalpiDevice;

/// Shared application state accessible to all handlers
#[derive(Clone)]
pub struct AppState {
    /// I2C device interface (mutex-protected for exclusive access)
    pub device: Arc<Mutex<HalpiDevice>>,
    /// Configuration (read-write lock for concurrent reads)
    pub config: Arc<RwLock<Config>>,
    /// Daemon version string
    pub version: &'static str,
    /// Number of LEDs on this hardware (derived from hardware version at startup)
    pub num_leds: usize,
    /// GID owning the control socket (resolved once at startup)
    pub socket_gid: u32,
}

impl AppState {
    /// Create new application state
    pub fn new(
        device: Arc<Mutex<HalpiDevice>>,
        config: Arc<RwLock<Config>>,
        num_leds: usize,
        socket_gid: u32,
    ) -> Self {
        Self {
            device,
            config,
            version: env!("CARGO_PKG_VERSION"),
            num_leds,
            socket_gid,
        }
    }
}

/// Run the HTTP server on a Unix socket
pub async fn run_server(state: AppState) -> anyhow::Result<()> {
    use std::path::PathBuf;
    use tokio::net::UnixListener;

    let socket_path = {
        let config = state.config.read().await;
        config
            .socket
            .clone()
            .unwrap_or_else(|| PathBuf::from("/run/halpid/halpid.sock"))
    };

    // Remove existing socket if it exists
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }

    // Create parent directory if it doesn't exist
    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let listener = UnixListener::bind(&socket_path)?;

    // Set socket permissions and group ownership
    setup_socket_permissions(&socket_path, state.socket_gid).await?;

    tracing::info!("HTTP server listening on {}", socket_path.display());

    let app = create_app(state);

    axum::serve(listener, app.into_make_service())
        .await
        .map_err(|e| anyhow::anyhow!("Server error: {}", e))?;

    Ok(())
}

/// Create the Axum application with all routes and middleware
pub fn create_app(state: AppState) -> Router {
    use super::handlers::{config, flash, health, shutdown, usb, values};

    Router::new()
        // Health and version endpoints
        .route("/", axum::routing::get(health::root))
        .route("/version", axum::routing::get(health::version))
        // Values endpoints
        .route("/values", axum::routing::get(values::get_all_values))
        .route("/values/{key}", axum::routing::get(values::get_value))
        // Configuration endpoints
        .route("/config", axum::routing::get(config::get_all_config))
        .route(
            "/config/{key}",
            axum::routing::get(config::get_config).put(config::put_config),
        )
        // Shutdown and standby endpoints
        .route("/shutdown", axum::routing::post(shutdown::post_shutdown))
        .route("/standby", axum::routing::post(shutdown::post_standby))
        // USB port control endpoints
        .route(
            "/usb",
            axum::routing::get(usb::get_all_usb).put(usb::put_all_usb),
        )
        .route(
            "/usb/{port}",
            axum::routing::get(usb::get_usb).put(usb::put_usb),
        )
        // Firmware upload endpoint
        .route("/flash", axum::routing::post(flash::post_flash))
        // Add tracing middleware
        .layer(TraceLayer::new_for_http())
        // Add shared state
        .with_state(state)
}

/// Resolve a group name to its GID.
///
/// Wraps the non-reentrant `getgrnam(3)`. Call this from a single thread before
/// spawning the socket tasks: concurrent `getgrnam` calls race on libc's shared
/// getgrent state and, under musl, corrupt the allocator (heap free of a bad
/// pointer → SIGSEGV).
#[cfg(unix)]
pub fn resolve_group_gid(group_name: &str) -> Result<u32, AppError> {
    use std::ffi::CString;

    let group_name_c = CString::new(group_name).map_err(|_| ServerError::ChangeGroupFailed {
        group: group_name.to_string(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid group name"),
    })?;

    let grp = unsafe { libc::getgrnam(group_name_c.as_ptr()) };
    if grp.is_null() {
        return Err(ServerError::ChangeGroupFailed {
            group: group_name.to_string(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "group not found"),
        }
        .into());
    }

    Ok(unsafe { (*grp).gr_gid })
}

/// Set Unix socket permissions and group ownership.
///
/// Takes a pre-resolved GID (see [`resolve_group_gid`]) rather than a group name,
/// so no `getgrnam` lookup happens here — keeping the concurrently-spawned socket
/// tasks free of that non-reentrant call.
#[cfg(unix)]
pub async fn setup_socket_permissions(socket_path: &Path, gid: u32) -> Result<(), AppError> {
    use std::ffi::CString;
    use std::os::unix::fs::PermissionsExt;

    // Set permissions to 0660 (rw-rw----)
    let permissions = std::fs::Permissions::from_mode(0o660);
    std::fs::set_permissions(socket_path, permissions).map_err(|e| {
        ServerError::SetPermissionsFailed {
            path: socket_path.to_path_buf(),
            source: e,
        }
    })?;

    // Set group ownership, keeping the current owning user.
    let uid = unsafe { libc::getuid() };
    let path_str = socket_path
        .to_str()
        .ok_or_else(|| ServerError::ChangeGroupFailed {
            group: gid.to_string(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "socket path is not valid UTF-8",
            ),
        })?;
    let path_c = CString::new(path_str).map_err(|_| ServerError::ChangeGroupFailed {
        group: gid.to_string(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid path"),
    })?;

    let result = unsafe { libc::chown(path_c.as_ptr(), uid, gid) };
    if result != 0 {
        return Err(ServerError::ChangeGroupFailed {
            group: gid.to_string(),
            source: std::io::Error::last_os_error(),
        }
        .into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_creation() {
        // Skip test if I2C hardware not available
        let device = match HalpiDevice::new(1, 0x6D) {
            Ok(d) => Arc::new(Mutex::new(d)),
            Err(_) => return,
        };
        let config = Arc::new(RwLock::new(Config::default()));
        let state = AppState::new(device, config, halpi_common::protocol::DEFAULT_NUM_LEDS, 0);

        assert_eq!(state.version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn test_create_app() {
        // Skip test if I2C hardware not available
        let device = match HalpiDevice::new(1, 0x6D) {
            Ok(d) => Arc::new(Mutex::new(d)),
            Err(_) => return,
        };
        let config = Arc::new(RwLock::new(Config::default()));
        let state = AppState::new(device, config, halpi_common::protocol::DEFAULT_NUM_LEDS, 0);

        let _app = create_app(state);
        // If this compiles and runs, the router is created successfully
    }
}
