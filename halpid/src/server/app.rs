//! Axum application setup and shared state

use axum::Router;
use halpi_common::config::Config;
use halpi_common::error::{AppError, ServerError};
use std::path::Path;
use std::sync::Arc;
#[cfg(target_os = "linux")]
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tower_http::trace::TraceLayer;

#[cfg(target_os = "linux")]
use crate::i2c::device::HalpiDevice;

/// Shared application state accessible to all handlers
#[derive(Clone)]
pub struct AppState {
    /// I2C device interface (mutex-protected for exclusive access)
    #[cfg(target_os = "linux")]
    pub device: Arc<Mutex<HalpiDevice>>,
    /// Configuration (read-write lock for concurrent reads)
    pub config: Arc<RwLock<Config>>,
    /// Daemon version string
    pub version: &'static str,
}

impl AppState {
    /// Create new application state
    #[cfg(target_os = "linux")]
    pub fn new(device: HalpiDevice, config: Config) -> Self {
        Self {
            device: Arc::new(Mutex::new(device)),
            config: Arc::new(RwLock::new(config)),
            version: env!("CARGO_PKG_VERSION"),
        }
    }
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
#[cfg(target_os = "linux")]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_creation() {
        // Skip test if I2C hardware not available
        let device = match HalpiDevice::new(1, 0x6D) {
            Ok(d) => d,
            Err(_) => return,
        };
        let config = Config::default();
        let state = AppState::new(device, config);

        assert_eq!(state.version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn test_create_app() {
        // Skip test if I2C hardware not available
        let device = match HalpiDevice::new(1, 0x6D) {
            Ok(d) => d,
            Err(_) => return,
        };
        let config = Config::default();
        let state = AppState::new(device, config);

        let _app = create_app(state);
        // If this compiles and runs, the router is created successfully
    }
}
