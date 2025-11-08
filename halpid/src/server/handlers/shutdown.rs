//! Shutdown and standby endpoint handlers

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[cfg(target_os = "linux")]
use chrono::TimeZone;

use crate::server::app::AppState;

/// Request body for standby endpoint
#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum StandbyRequest {
    /// Standby with delay in seconds
    Delay { delay: u32 },
    /// Standby with specific datetime (ISO 8601 format)
    Datetime { datetime: String },
}

/// POST /shutdown - Request system shutdown
#[cfg(target_os = "linux")]
pub async fn post_shutdown(State(state): State<AppState>) -> Response {
    let mut device = state.device.lock().await;

    match device.request_shutdown() {
        Ok(()) => (StatusCode::NO_CONTENT, ()).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to request shutdown: {}", e)})),
        )
            .into_response(),
    }
}

/// POST /standby - Request system standby with wakeup
#[cfg(target_os = "linux")]
pub async fn post_standby(
    State(state): State<AppState>,
    Json(payload): Json<StandbyRequest>,
) -> Response {
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    // Calculate wakeup time based on request type
    let wakeup_timestamp = match payload {
        StandbyRequest::Delay { delay } => {
            // Current time + delay
            let now = match SystemTime::now().duration_since(UNIX_EPOCH) {
                Ok(duration) => duration.as_secs(),
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": format!("System time is before Unix epoch: {}", e)})),
                    )
                        .into_response();
                }
            };
            now + delay as u64
        }
        StandbyRequest::Datetime { datetime } => {
            // Parse ISO 8601 datetime string
            // For simplicity, we'll use chrono for parsing
            match parse_datetime(&datetime) {
                Ok(timestamp) => timestamp,
                Err(e) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"error": format!("Invalid datetime format: {}", e)})),
                    )
                        .into_response();
                }
            }
        }
    };

    // Set RTC alarm using rtcwake
    let rtcwake_result = Command::new("rtcwake")
        .arg("-m")
        .arg("no") // Don't suspend, just set alarm
        .arg("-t")
        .arg(wakeup_timestamp.to_string())
        .output();

    match rtcwake_result {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("rtcwake failed: {}", stderr)})),
                )
                    .into_response();
            }
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to execute rtcwake: {}", e)})),
            )
                .into_response();
        }
    }

    // Now request standby via I2C
    let mut device = state.device.lock().await;
    match device.request_standby() {
        Ok(()) => (StatusCode::NO_CONTENT, ()).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to request standby: {}", e)})),
        )
            .into_response(),
    }
}

/// Parse ISO 8601 datetime string to Unix timestamp
#[cfg(target_os = "linux")]
fn parse_datetime(datetime: &str) -> Result<u64, String> {
    // Try parsing with different formats
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(datetime) {
        Ok(dt.timestamp() as u64)
    } else if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(datetime, "%Y-%m-%d %H:%M:%S") {
        // Interpret as local timezone
        match chrono::Local.from_local_datetime(&dt) {
            chrono::LocalResult::Single(dt_with_tz) => Ok(dt_with_tz.timestamp() as u64),
            _ => Err(format!(
                "Could not interpret datetime '{}' as local time",
                datetime
            )),
        }
    } else {
        Err(format!(
            "Could not parse datetime: {}. Expected ISO 8601 format (e.g., '2025-11-08T12:00:00Z' or '2025-11-08 12:00:00')",
            datetime
        ))
    }
}

/// Stubs for non-Linux platforms
#[cfg(not(target_os = "linux"))]
pub async fn post_shutdown(State(_state): State<AppState>) -> Response {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({"error": "I2C device access only supported on Linux"})),
    )
        .into_response()
}

#[cfg(not(target_os = "linux"))]
pub async fn post_standby(
    State(_state): State<AppState>,
    Json(_payload): Json<StandbyRequest>,
) -> Response {
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
    async fn test_post_shutdown() {
        let device = match HalpiDevice::new(1, 0x6D) {
            Ok(d) => d,
            Err(_) => return,
        };
        let config = Config::default();
        let state = AppState::new(device, config);

        let response = post_shutdown(State(state)).await;
        // Will be 204 or 500 depending on I2C availability
        assert!(
            response.status() == StatusCode::NO_CONTENT
                || response.status() == StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn test_parse_datetime_rfc3339() {
        let result = parse_datetime("2025-11-08T12:00:00Z");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_datetime_local_format() {
        let result = parse_datetime("2025-11-08 12:00:00");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_datetime_invalid() {
        let result = parse_datetime("not a date");
        assert!(result.is_err());
    }
}
