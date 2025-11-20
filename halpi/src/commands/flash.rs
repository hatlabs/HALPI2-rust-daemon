//! Firmware flash command implementation

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use crate::client::HalpiClient;

/// Upload firmware to the device
pub async fn flash(firmware_path: &str) -> Result<()> {
    // Validate file exists
    let path = Path::new(firmware_path);
    if !path.exists() {
        anyhow::bail!("Firmware file not found: {}", firmware_path);
    }

    if !path.is_file() {
        anyhow::bail!("Path is not a file: {}", firmware_path);
    }

    // Get the filename for the multipart form
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("firmware.bin");

    // Read firmware file
    println!("Reading firmware file: {}", firmware_path);
    let firmware_data = fs::read(path)
        .with_context(|| format!("Failed to read firmware file: {}", firmware_path))?;

    let file_size = firmware_data.len();
    println!("Firmware size: {} bytes", file_size);

    if firmware_data.is_empty() {
        anyhow::bail!("Firmware file is empty");
    }

    // Upload firmware
    println!("Uploading firmware to device...");
    let client = HalpiClient::new();
    client.upload_firmware(firmware_data, filename).await?;

    println!("Firmware uploaded successfully");

    Ok(())
}
