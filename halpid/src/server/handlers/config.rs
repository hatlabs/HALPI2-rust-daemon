//! Configuration endpoint handlers

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::server::app::AppState;

/// GET /config - Get all configuration values
pub async fn get_all_config(State(state): State<AppState>) -> Response {
    let config = state.config.read().await;

    let config_json = json!({
        "i2c_bus": config.i2c_bus,
        "i2c_addr": format!("0x{:02X}", config.i2c_addr),
        "blackout_time_limit": config.blackout_time_limit,
        "blackout_voltage_limit": config.blackout_voltage_limit,
        "socket": config.socket.as_ref().map(|p| p.to_string_lossy().to_string()),
        "socket_group": config.socket_group,
        "poweroff": config.poweroff,
    });

    (StatusCode::OK, Json(config_json)).into_response()
}

/// GET /config/:key - Get a specific configuration value
pub async fn get_config(State(state): State<AppState>, Path(key): Path<String>) -> Response {
    let config = state.config.read().await;

    let value = match key.as_str() {
        "i2c_bus" => Some(json!(config.i2c_bus)),
        "i2c_addr" => Some(json!(format!("0x{:02X}", config.i2c_addr))),
        "blackout_time_limit" => Some(json!(config.blackout_time_limit)),
        "blackout_voltage_limit" => Some(json!(config.blackout_voltage_limit)),
        "socket" => Some(json!(
            config
                .socket
                .as_ref()
                .map(|p| p.to_string_lossy().to_string())
        )),
        "socket_group" => Some(json!(config.socket_group)),
        "poweroff" => Some(json!(config.poweroff)),
        _ => None,
    };

    match value {
        Some(v) => (StatusCode::OK, Json(v)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("Unknown config key: {}", key)})),
        )
            .into_response(),
    }
}

/// PUT /config/:key - Update a specific configuration value
///
/// Note: Daemon configuration is read from file and not modified at runtime.
/// This endpoint returns METHOD_NOT_ALLOWED for API compatibility.
pub async fn put_config(
    State(_state): State<AppState>,
    Path(_key): Path<String>,
    Json(_payload): Json<serde_json::Value>,
) -> Response {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        Json(json!({"error": "Configuration is read-only, modify /etc/halpid/halpid.conf and restart daemon"})),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i2c::device::HalpiDevice;
    use halpi_common::config::Config;
    use std::sync::Arc;
    use tokio::sync::{Mutex, RwLock};

    #[tokio::test]
    async fn test_get_all_config() {
        let device = match HalpiDevice::new(1, 0x6D) {
            Ok(d) => Arc::new(Mutex::new(d)),
            Err(_) => return,
        };
        let config = Arc::new(RwLock::new(Config::default()));
        let state = AppState::new(device, config);

        let response = get_all_config(State(state)).await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_config_valid_key() {
        let device = match HalpiDevice::new(1, 0x6D) {
            Ok(d) => Arc::new(Mutex::new(d)),
            Err(_) => return,
        };
        let config = Arc::new(RwLock::new(Config::default()));
        let state = AppState::new(device, config);

        let response = get_config(State(state), Path("i2c_bus".to_string())).await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_config_invalid_key() {
        let device = match HalpiDevice::new(1, 0x6D) {
            Ok(d) => Arc::new(Mutex::new(d)),
            Err(_) => return,
        };
        let config = Arc::new(RwLock::new(Config::default()));
        let state = AppState::new(device, config);

        let response = get_config(State(state), Path("invalid_key".to_string())).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
