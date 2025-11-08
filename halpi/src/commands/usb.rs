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

/// Enable a USB port
pub async fn usb_enable(port: &str) -> Result<()> {
    let client = HalpiClient::new();

    if port == "all" {
        // Enable all ports
        for i in 0..4 {
            client.set_usb_port(i, true).await?;
        }
        println!("All USB ports enabled");
    } else {
        // Enable specific port
        let port_num: u8 = port
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid port number: {}. Must be 0-3 or 'all'", port))?;

        if port_num > 3 {
            anyhow::bail!("Invalid port number: {}. Must be 0-3", port_num);
        }

        client.set_usb_port(port_num, true).await?;
        println!("USB port {} enabled", port_num);
    }

    Ok(())
}

/// Disable a USB port
pub async fn usb_disable(port: &str) -> Result<()> {
    let client = HalpiClient::new();

    if port == "all" {
        // Disable all ports
        for i in 0..4 {
            client.set_usb_port(i, false).await?;
        }
        println!("All USB ports disabled");
    } else {
        // Disable specific port
        let port_num: u8 = port
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid port number: {}. Must be 0-3 or 'all'", port))?;

        if port_num > 3 {
            anyhow::bail!("Invalid port number: {}. Must be 0-3", port_num);
        }

        client.set_usb_port(port_num, false).await?;
        println!("USB port {} disabled", port_num);
    }

    Ok(())
}
