//! Values endpoint handlers for sensor readings and device information

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
#[cfg(target_os = "linux")]
use halpi_common::protocol::kelvin_to_celsius;
#[cfg(target_os = "linux")]
use serde_json::Value;
use serde_json::json;

use crate::server::app::AppState;

/// GET /values - Get all sensor readings and device information
#[cfg(target_os = "linux")]
pub async fn get_all_values(State(state): State<AppState>) -> Response {
    // Acquire device lock and read all values at once to minimize lock time
    let device = state.device.lock().await;

    // Read all measurements
    let measurements = match device.get_measurements() {
        Ok(m) => m,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    // Read version information
    let hardware_version = device
        .get_hardware_version()
        .unwrap_or_else(|_| halpi_common::types::Version::from_bytes([255, 0, 0, 0]));
    let firmware_version = device
        .get_firmware_version()
        .unwrap_or_else(|_| halpi_common::types::Version::from_bytes([255, 0, 0, 0]));

    // Read device ID
    let device_id = device
        .get_device_id()
        .unwrap_or_else(|_| "0000000000000000".to_string());

    // Release lock
    drop(device);

    // Build response JSON
    let response_json = json!({
        "daemon_version": state.version,
        "hardware_version": hardware_version.to_string(),
        "firmware_version": firmware_version.to_string(),
        "device_id": device_id,
        "V_in": measurements.dcin_voltage,
        "V_cap": measurements.supercap_voltage,
        "I_in": measurements.input_current,
        "T_mcu": kelvin_to_celsius(measurements.mcu_temperature),
        "T_pcb": kelvin_to_celsius(measurements.pcb_temperature),
        "state": measurements.power_state.name(),
        "watchdog_elapsed": measurements.watchdog_elapsed,
    });

    (StatusCode::OK, Json(response_json)).into_response()
}

/// GET /values/:key - Get a specific value by key
#[cfg(target_os = "linux")]
pub async fn get_value(State(state): State<AppState>, Path(key): Path<String>) -> Response {
    // Match the requested key and only lock device if needed
    match key.as_str() {
        "daemon_version" => {
            let value = json!(state.version);
            (StatusCode::OK, Json(value)).into_response()
        }
        "hardware_version" | "firmware_version" | "device_id" | "V_in" | "V_cap" | "I_in"
        | "T_mcu" | "T_pcb" | "state" | "watchdog_elapsed" => {
            let device = state.device.lock().await;

            // Read only the data needed for the requested key
            let value: Result<Value, String> = match key.as_str() {
                "hardware_version" => device
                    .get_hardware_version()
                    .map(|v| json!(v.to_string()))
                    .or_else(|_| {
                        Ok(json!(
                            halpi_common::types::Version::from_bytes([255, 0, 0, 0]).to_string()
                        ))
                    }),
                "firmware_version" => device
                    .get_firmware_version()
                    .map(|v| json!(v.to_string()))
                    .or_else(|_| {
                        Ok(json!(
                            halpi_common::types::Version::from_bytes([255, 0, 0, 0]).to_string()
                        ))
                    }),
                "device_id" => device
                    .get_device_id()
                    .map(|id| json!(id))
                    .or_else(|_| Ok(json!("0000000000000000"))),
                "V_in" | "V_cap" | "I_in" | "T_mcu" | "T_pcb" | "state" | "watchdog_elapsed" => {
                    match device.get_measurements() {
                        Ok(m) => Ok(match key.as_str() {
                            "V_in" => json!(m.dcin_voltage),
                            "V_cap" => json!(m.supercap_voltage),
                            "I_in" => json!(m.input_current),
                            "T_mcu" => json!(kelvin_to_celsius(m.mcu_temperature)),
                            "T_pcb" => json!(kelvin_to_celsius(m.pcb_temperature)),
                            "state" => json!(m.power_state.name()),
                            "watchdog_elapsed" => json!(m.watchdog_elapsed),
                            _ => unreachable!(),
                        }),
                        Err(e) => Err(e.to_string()),
                    }
                }
                _ => unreachable!(),
            };

            drop(device);

            match value {
                Ok(v) => (StatusCode::OK, Json(v)).into_response(),
                Err(e) => {
                    (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e}))).into_response()
                }
            }
        }
        _ => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("Unknown key: {}", key)})),
        )
            .into_response(),
    }
}

/// Stub for non-Linux platforms
#[cfg(not(target_os = "linux"))]
pub async fn get_all_values(State(_state): State<AppState>) -> Response {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({"error": "I2C device access only supported on Linux"})),
    )
        .into_response()
}

/// Stub for non-Linux platforms
#[cfg(not(target_os = "linux"))]
pub async fn get_value(State(_state): State<AppState>, Path(_key): Path<String>) -> Response {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({"error": "I2C device access only supported on Linux"})),
    )
        .into_response()
}

#[cfg(test)]
#[cfg(target_os = "linux")]
mod tests {
    use super::*;
    use crate::i2c::device::HalpiDevice;
    use halpi_common::config::Config;

    #[tokio::test]
    async fn test_get_all_values_structure() {
        let device = HalpiDevice::new(1, 0x6D).unwrap();
        let config = Config::default();
        let state = AppState::new(device, config);

        let response = get_all_values(State(state)).await;
        // Response will be 500 if no I2C device, but should be a valid response structure
        assert!(
            response.status() == StatusCode::OK
                || response.status() == StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[tokio::test]
    async fn test_get_value_unknown_key() {
        let device = HalpiDevice::new(1, 0x6D).unwrap();
        let config = Config::default();
        let state = AppState::new(device, config);

        let response = get_value(State(state), Path("invalid_key".to_string())).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
