//! Firmware upload endpoint handler

use axum::Json;
use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::server::app::AppState;

/// POST /flash - Upload firmware to device
pub async fn post_flash(State(state): State<AppState>, mut multipart: Multipart) -> Response {
    // Extract firmware file from multipart form data
    let firmware_data = match extract_firmware(&mut multipart).await {
        Ok(data) => data,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": format!("Failed to extract firmware: {}", e)})),
            )
                .into_response();
        }
    };

    if firmware_data.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Firmware file is empty"})),
        )
            .into_response();
    }

    // Acquire device lock for the entire upload process
    let mut device = state.device.lock().await;

    // Upload firmware using high-level method with progress tracking
    if let Err(e) = device.upload_firmware(&firmware_data, |_written, _total| {
        // Progress callback - silent for now
        // Could add tracing::debug!() here for verbose logging
    }) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to upload firmware: {}", e)})),
        )
            .into_response();
    }

    (StatusCode::NO_CONTENT, ()).into_response()
}

/// Extract firmware data from multipart form
async fn extract_firmware(multipart: &mut Multipart) -> Result<Vec<u8>, String> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| format!("Failed to read multipart field: {}", e))?
    {
        if let Some(name) = field.name()
            && name == "firmware"
        {
            let data = field
                .bytes()
                .await
                .map_err(|e| format!("Failed to read firmware data: {}", e))?;
            return Ok(data.to_vec());
        }
    }

    Err("No 'firmware' field found in multipart form".to_string())
}

#[cfg(test)]
mod tests {
    // No tests needed for flash handler - integration testing required with actual hardware
}
