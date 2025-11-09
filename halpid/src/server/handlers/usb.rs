//! USB port control endpoint handlers

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::server::app::AppState;

/// GET /usb - Get all USB port states
pub async fn get_all_usb(State(state): State<AppState>) -> Response {
    let mut device = state.device.lock().await;

    match device.get_usb_port_state() {
        Ok(port_bits) => {
            let usb_json = json!({
                "usb0": (port_bits & 0x01) != 0,
                "usb1": (port_bits & 0x02) != 0,
                "usb2": (port_bits & 0x04) != 0,
                "usb3": (port_bits & 0x08) != 0,
            });
            (StatusCode::OK, Json(usb_json)).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to get USB port states: {}", e)})),
        )
            .into_response(),
    }
}

/// GET /usb/:port - Get specific USB port state
pub async fn get_usb(State(state): State<AppState>, Path(port): Path<u8>) -> Response {
    if port > 3 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid port number, must be 0-3"})),
        )
            .into_response();
    }

    let mut device = state.device.lock().await;

    match device.get_usb_port_state() {
        Ok(port_bits) => {
            let enabled = (port_bits & (1 << port)) != 0;
            (StatusCode::OK, Json(json!(enabled))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to get USB port state: {}", e)})),
        )
            .into_response(),
    }
}

/// PUT /usb - Set all USB port states
///
/// Only updates the ports specified in the payload. Unspecified ports retain their current state.
pub async fn put_all_usb(
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> Response {
    // Parse JSON object with usb0-usb3 fields
    let usb0 = payload.get("usb0").and_then(|v| v.as_bool());
    let usb1 = payload.get("usb1").and_then(|v| v.as_bool());
    let usb2 = payload.get("usb2").and_then(|v| v.as_bool());
    let usb3 = payload.get("usb3").and_then(|v| v.as_bool());

    let mut device = state.device.lock().await;

    // Read current port state
    let current_bits = match device.get_usb_port_state() {
        Ok(bits) => bits,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to get current USB port states: {}", e)})),
            )
                .into_response();
        }
    };

    // Update only specified fields
    let mut port_bits = current_bits;
    if let Some(val) = usb0 {
        if val {
            port_bits |= 0x01;
        } else {
            port_bits &= !0x01;
        }
    }
    if let Some(val) = usb1 {
        if val {
            port_bits |= 0x02;
        } else {
            port_bits &= !0x02;
        }
    }
    if let Some(val) = usb2 {
        if val {
            port_bits |= 0x04;
        } else {
            port_bits &= !0x04;
        }
    }
    if let Some(val) = usb3 {
        if val {
            port_bits |= 0x08;
        } else {
            port_bits &= !0x08;
        }
    }

    match device.set_usb_port_state(port_bits) {
        Ok(()) => (StatusCode::NO_CONTENT, ()).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to set USB port states: {}", e)})),
        )
            .into_response(),
    }
}

/// PUT /usb/:port - Set specific USB port state
pub async fn put_usb(
    State(state): State<AppState>,
    Path(port): Path<u8>,
    Json(payload): Json<bool>,
) -> Response {
    if port > 3 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid port number, must be 0-3"})),
        )
            .into_response();
    }

    let mut device = state.device.lock().await;

    // Read current state
    let current_bits = match device.get_usb_port_state() {
        Ok(bits) => bits,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to get current USB port state: {}", e)})),
            )
                .into_response();
        }
    };

    // Update specific bit
    let new_bits = if payload {
        current_bits | (1 << port)
    } else {
        current_bits & !(1 << port)
    };

    // Write back
    match device.set_usb_port_state(new_bits) {
        Ok(()) => (StatusCode::NO_CONTENT, ()).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to set USB port state: {}", e)})),
        )
            .into_response(),
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
    async fn test_get_all_usb() {
        let device = match HalpiDevice::new(1, 0x6D) {
            Ok(d) => Arc::new(Mutex::new(d)),
            Err(_) => return,
        };
        let config = Arc::new(RwLock::new(Config::default()));
        let state = AppState::new(device, config);

        let response = get_all_usb(State(state)).await;
        assert!(
            response.status() == StatusCode::OK
                || response.status() == StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[tokio::test]
    async fn test_get_usb_valid_port() {
        let device = match HalpiDevice::new(1, 0x6D) {
            Ok(d) => Arc::new(Mutex::new(d)),
            Err(_) => return,
        };
        let config = Arc::new(RwLock::new(Config::default()));
        let state = AppState::new(device, config);

        let response = get_usb(State(state), Path(0)).await;
        assert!(
            response.status() == StatusCode::OK
                || response.status() == StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[tokio::test]
    async fn test_get_usb_invalid_port() {
        let device = match HalpiDevice::new(1, 0x6D) {
            Ok(d) => Arc::new(Mutex::new(d)),
            Err(_) => return,
        };
        let config = Arc::new(RwLock::new(Config::default()));
        let state = AppState::new(device, config);

        let response = get_usb(State(state), Path(4)).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
