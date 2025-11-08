//! Firmware upload endpoint handler

use axum::Json;
use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::server::app::AppState;

/// POST /flash - Upload firmware to device
#[cfg(target_os = "linux")]
pub async fn post_flash(State(state): State<AppState>, mut multipart: Multipart) -> Response {
    // Block size for firmware upload (4KB)
    const FLASH_BLOCK_SIZE: usize = 4096;

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

    // Start DFU process
    if let Err(e) = device.start_dfu(firmware_data.len() as u32) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to start DFU: {}", e)})),
        )
            .into_response();
    }

    // Upload firmware in 4KB blocks
    let num_blocks = (firmware_data.len() + FLASH_BLOCK_SIZE - 1) / FLASH_BLOCK_SIZE;

    for block_num in 0..num_blocks {
        let start = block_num * FLASH_BLOCK_SIZE;
        let end = std::cmp::min(start + FLASH_BLOCK_SIZE, firmware_data.len());
        let block_data = &firmware_data[start..end];

        // Upload block with retries
        if let Err(e) = device.upload_block(block_num as u16, block_data) {
            // Abort DFU on error
            let _ = device.abort_dfu();
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": format!("Failed to upload block {}: {}", block_num, e)
                })),
            )
                .into_response();
        }
    }

    // Commit DFU
    if let Err(e) = device.commit_dfu() {
        let _ = device.abort_dfu();
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to commit DFU: {}", e)})),
        )
            .into_response();
    }

    (StatusCode::NO_CONTENT, ()).into_response()
}

/// Extract firmware data from multipart form
#[cfg(target_os = "linux")]
async fn extract_firmware(multipart: &mut Multipart) -> Result<Vec<u8>, String> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| format!("Failed to read multipart field: {}", e))?
    {
        if let Some(name) = field.name() {
            if name == "firmware" {
                let data = field
                    .bytes()
                    .await
                    .map_err(|e| format!("Failed to read firmware data: {}", e))?;
                return Ok(data.to_vec());
            }
        }
    }

    Err("No 'firmware' field found in multipart form".to_string())
}

/// Stub for non-Linux platforms
#[cfg(not(target_os = "linux"))]
pub async fn post_flash(State(_state): State<AppState>, _multipart: Multipart) -> Response {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({"error": "I2C device access only supported on Linux"})),
    )
        .into_response()
}

#[cfg(test)]
#[cfg(target_os = "linux")]
mod tests {
    // No tests needed for flash handler - integration testing required with actual hardware
}
