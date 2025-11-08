//! Firmware flash command implementation

use anyhow::Result;
use std::path::Path;

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

    // For now, provide instructions to use curl or similar
    // Full multipart upload implementation requires additional dependencies
    println!("Firmware upload via CLI is not yet implemented.");
    println!();
    println!("To upload firmware, use curl:");
    println!(
        "  curl -X POST -F \"firmware=@{}\" --unix-socket /run/halpid/halpid.sock http://localhost/flash",
        firmware_path
    );
    println!();
    println!("Or access the daemon's HTTP API directly.");

    Ok(())
}
