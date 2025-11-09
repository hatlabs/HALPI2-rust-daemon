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
}

impl AppState {
    /// Create new application state
    pub fn new(device: Arc<Mutex<HalpiDevice>>, config: Arc<RwLock<Config>>) -> Self {
        Self {
            device,
            config,
            version: env!("CARGO_PKG_VERSION"),
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
        .route("/values/:key", axum::routing::get(values::get_value))
        // Configuration endpoints
        .route("/config", axum::routing::get(config::get_all_config))
        .route(
            "/config/:key",
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
            "/usb/:port",
            axum::routing::get(usb::get_usb).put(usb::put_usb),
        )
        // Firmware upload endpoint
        .route("/flash", axum::routing::post(flash::post_flash))
        // Add tracing middleware
        .layer(TraceLayer::new_for_http())
        // Add shared state
        .with_state(state)
}

/// Set Unix socket permissions and group ownership
#[cfg(unix)]
pub async fn setup_socket_permissions(
    socket_path: &Path,
    group_name: &str,
) -> Result<(), AppError> {
    use std::os::unix::fs::PermissionsExt;

    // Set permissions to 0660 (rw-rw----)
    let permissions = std::fs::Permissions::from_mode(0o660);
    std::fs::set_permissions(socket_path, permissions).map_err(|e| {
        ServerError::SetPermissionsFailed {
            path: socket_path.to_path_buf(),
            source: e,
        }
    })?;

    // Set group ownership
    set_socket_group(socket_path, group_name)?;

    Ok(())
}

/// Set the group ownership of the socket file
#[cfg(unix)]
fn set_socket_group(socket_path: &Path, group_name: &str) -> Result<(), AppError> {
    use std::ffi::CString;

    // Get group ID from group name
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

    let gid = unsafe { (*grp).gr_gid };

    // Get current user ID (don't change ownership)
    let uid = unsafe { libc::getuid() };

    // Change ownership - handle invalid UTF-8 in path
    let path_str = socket_path
        .to_str()
        .ok_or_else(|| ServerError::ChangeGroupFailed {
            group: group_name.to_string(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "socket path is not valid UTF-8",
            ),
        })?;
    let path_c = CString::new(path_str).map_err(|_| ServerError::ChangeGroupFailed {
        group: group_name.to_string(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid path"),
    })?;

    let result = unsafe { libc::chown(path_c.as_ptr(), uid, gid) };
    if result != 0 {
        return Err(ServerError::ChangeGroupFailed {
            group: group_name.to_string(),
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
        let state = AppState::new(device, config);

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
        let state = AppState::new(device, config);

        let _app = create_app(state);
        // If this compiles and runs, the router is created successfully
    }
}
