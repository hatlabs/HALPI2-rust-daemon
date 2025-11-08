//! Health and version endpoint handlers

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::server::app::AppState;

/// GET / - Root health check endpoint
///
/// Returns plain text "This is halpid!\n" for compatibility with Python version
pub async fn root() -> Response {
    (StatusCode::OK, "This is halpid!\n").into_response()
}

/// GET /version - Version information endpoint
///
/// Returns JSON object with daemon version
pub async fn version(State(state): State<AppState>) -> Response {
    let version_json = json!({
        "daemon_version": state.version
    });

    (StatusCode::OK, Json(version_json)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_root_endpoint() {
        let response = root().await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn test_version_endpoint() {
        use crate::i2c::device::HalpiDevice;
        use halpi_common::config::Config;

        // Skip test if I2C hardware not available
        let device = match HalpiDevice::new(1, 0x6D) {
            Ok(d) => d,
            Err(_) => return,
        };
        let config = Config::default();
        let state = AppState::new(device, config);

        let response = version(State(state)).await;
        assert_eq!(response.status(), StatusCode::OK);
    }
}
