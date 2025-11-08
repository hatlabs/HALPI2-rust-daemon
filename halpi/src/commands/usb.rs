//! USB port control command implementation

use anyhow::Result;

use crate::client::HalpiClient;

/// Display all USB port states
pub async fn usb_status() -> Result<()> {
    let client = HalpiClient::new();
    let ports = client.get_usb_ports().await?;

    println!();
    println!("USB Port States:");
    for i in 0..4 {
        let key = format!("usb{}", i);
        if let Some(&enabled) = ports.get(&key) {
            let status = if enabled { "enabled" } else { "disabled" };
            println!("  Port {}: {}", i, status);
        }
    }
    println!();

    Ok(())
}

/// Helper function to set USB port state
async fn set_usb_port_state(port: &str, enabled: bool) -> Result<()> {
    let client = HalpiClient::new();

    if port == "all" {
        // Set state for all ports
        for i in 0..4 {
            client.set_usb_port(i, enabled).await?;
        }
        let status = if enabled { "enabled" } else { "disabled" };
        println!("All USB ports {}", status);
    } else {
        // Set state for specific port
        let port_num: u8 = port
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid port number: {}. Must be 0-3 or 'all'", port))?;

        if port_num > 3 {
            anyhow::bail!("Invalid port number: {}. Must be 0-3", port_num);
        }

        client.set_usb_port(port_num, enabled).await?;
        let status = if enabled { "enabled" } else { "disabled" };
        println!("USB port {} {}", port_num, status);
    }

    Ok(())
}

/// Enable a USB port
pub async fn usb_enable(port: &str) -> Result<()> {
    set_usb_port_state(port, true).await
}

/// Disable a USB port
pub async fn usb_disable(port: &str) -> Result<()> {
    set_usb_port_state(port, false).await
}
