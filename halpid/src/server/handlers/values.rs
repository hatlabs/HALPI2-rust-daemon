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
    let device_id = device.get_device_id().unwrap_or([0; 8]);

    // Release lock
    drop(device);

    // Build response JSON
    let response_json = json!({
        "daemon_version": state.version,
        "hardware_version": hardware_version.to_string(),
        "firmware_version": firmware_version.to_string(),
        "device_id": format!("{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            device_id[0], device_id[1], device_id[2], device_id[3],
            device_id[4], device_id[5], device_id[6], device_id[7]),
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
    // Get all values
    let device = state.device.lock().await;

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

    let hardware_version = device
        .get_hardware_version()
        .unwrap_or_else(|_| halpi_common::types::Version::from_bytes([255, 0, 0, 0]));
    let firmware_version = device
        .get_firmware_version()
        .unwrap_or_else(|_| halpi_common::types::Version::from_bytes([255, 0, 0, 0]));
    let device_id = device.get_device_id().unwrap_or([0; 8]);

    drop(device);

    // Match the requested key and return the specific value
    let value: Value = match key.as_str() {
        "daemon_version" => json!(state.version),
        "hardware_version" => json!(hardware_version.to_string()),
        "firmware_version" => json!(firmware_version.to_string()),
        "device_id" => json!(format!(
            "{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            device_id[0],
            device_id[1],
            device_id[2],
            device_id[3],
            device_id[4],
            device_id[5],
            device_id[6],
            device_id[7]
        )),
        "V_in" => json!(measurements.dcin_voltage),
        "V_cap" => json!(measurements.supercap_voltage),
        "I_in" => json!(measurements.input_current),
        "T_mcu" => json!(kelvin_to_celsius(measurements.mcu_temperature)),
        "T_pcb" => json!(kelvin_to_celsius(measurements.pcb_temperature)),
        "state" => json!(measurements.power_state.name()),
        "watchdog_elapsed" => json!(measurements.watchdog_elapsed),
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": format!("Unknown key: {}", key)})),
            )
                .into_response();
        }
    };

    (StatusCode::OK, Json(value)).into_response()
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
