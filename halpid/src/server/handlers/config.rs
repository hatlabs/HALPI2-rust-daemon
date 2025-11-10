//! Configuration endpoint handlers
//!
//! These endpoints read/write controller configuration from I2C registers,
//! NOT the daemon's configuration file.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::server::app::AppState;

/// GET /config - Get all configuration values from controller
pub async fn get_all_config(State(state): State<AppState>) -> Response {
    let mut device = state.device.lock().await;

    // Read all configuration values from controller registers
    let watchdog_timeout = device.get_watchdog_timeout().unwrap_or(0);
    let power_on_threshold = device.get_power_on_threshold().unwrap_or(0.0);
    let solo_power_off_threshold = device.get_solo_power_off_threshold().unwrap_or(0.0);
    let led_brightness = device.get_led_brightness().unwrap_or(0);
    let auto_restart = device.get_auto_restart().unwrap_or(false);
    let solo_depleting_timeout = device.get_solo_depleting_timeout().unwrap_or(0);

    drop(device);

    let config_json = json!({
        "watchdog_timeout": watchdog_timeout as f64 / 1000.0, // Convert ms to seconds
        "power_on_threshold": power_on_threshold,
        "solo_power_off_threshold": solo_power_off_threshold,
        "led_brightness": led_brightness,
        "auto_restart": auto_restart,
        "solo_depleting_timeout": solo_depleting_timeout as f64 / 1000.0, // Convert ms to seconds
    });

    (StatusCode::OK, Json(config_json)).into_response()
}

/// GET /config/:key - Get a specific configuration value from controller
pub async fn get_config(State(state): State<AppState>, Path(key): Path<String>) -> Response {
    let mut device = state.device.lock().await;

    let value = match key.as_str() {
        "watchdog_timeout" => device
            .get_watchdog_timeout()
            .map(|v| json!(v as f64 / 1000.0))
            .ok(),
        "power_on_threshold" => device.get_power_on_threshold().map(|v| json!(v)).ok(),
        "solo_power_off_threshold" => device.get_solo_power_off_threshold().map(|v| json!(v)).ok(),
        "led_brightness" => device.get_led_brightness().map(|v| json!(v)).ok(),
        "auto_restart" => device.get_auto_restart().map(|v| json!(v)).ok(),
        "solo_depleting_timeout" => device
            .get_solo_depleting_timeout()
            .map(|v| json!(v as f64 / 1000.0))
            .ok(),
        _ => None,
    };

    drop(device);

    match value {
        Some(v) => (StatusCode::OK, Json(v)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("Unknown config key: {}", key)})),
        )
            .into_response(),
    }
}

/// PUT /config/:key - Update a specific configuration value on controller
pub async fn put_config(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(payload): Json<serde_json::Value>,
) -> Response {
    let mut device = state.device.lock().await;

    let result = match key.as_str() {
        "watchdog_timeout" => {
            if let Some(value) = payload.as_f64() {
                let timeout_ms = (value * 1000.0) as u16;
                device
                    .set_watchdog_timeout(timeout_ms)
                    .map_err(|e| e.to_string())
            } else {
                Err("Invalid value type".to_string())
            }
        }
        "power_on_threshold" => {
            if let Some(value) = payload.as_f64() {
                device
                    .set_power_on_threshold(value as f32)
                    .map_err(|e| e.to_string())
            } else {
                Err("Invalid value type".to_string())
            }
        }
        "solo_power_off_threshold" => {
            if let Some(value) = payload.as_f64() {
                device
                    .set_solo_power_off_threshold(value as f32)
                    .map_err(|e| e.to_string())
            } else {
                Err("Invalid value type".to_string())
            }
        }
        "led_brightness" => {
            if let Some(value) = payload.as_u64() {
                device
                    .set_led_brightness(value as u8)
                    .map_err(|e| e.to_string())
            } else {
                Err("Invalid value type".to_string())
            }
        }
        "auto_restart" => {
            if let Some(value) = payload.as_bool() {
                device.set_auto_restart(value).map_err(|e| e.to_string())
            } else {
                Err("Invalid value type".to_string())
            }
        }
        "solo_depleting_timeout" => {
            if let Some(value) = payload.as_f64() {
                let timeout_ms = (value * 1000.0) as u32;
                device
                    .set_solo_depleting_timeout(timeout_ms)
                    .map_err(|e| e.to_string())
            } else {
                Err("Invalid value type".to_string())
            }
        }
        _ => Err(format!("Unknown config key: {}", key)),
    };

    drop(device);

    match result {
        Ok(_) => (StatusCode::OK, Json(json!({"status": "ok"}))).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({"error": e}))).into_response(),
    }
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
