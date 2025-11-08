//! Shutdown and standby command implementation

use anyhow::Result;

use crate::client::HalpiClient;

/// Request system shutdown
pub async fn shutdown() -> Result<()> {
    let client = HalpiClient::new();
    client.shutdown().await?;
    println!("Shutdown requested");
    Ok(())
}

/// Request system standby with delay
pub async fn standby_delay(delay_seconds: u32) -> Result<()> {
    let client = HalpiClient::new();
    client.standby_with_delay(delay_seconds).await?;
    println!("Standby requested with wakeup in {} seconds", delay_seconds);
    Ok(())
}

/// Request system standby with datetime
pub async fn standby_datetime(datetime: &str) -> Result<()> {
    let client = HalpiClient::new();
    client.standby_at_datetime(datetime).await?;
    println!("Standby requested with wakeup at {}", datetime);
    Ok(())
}
