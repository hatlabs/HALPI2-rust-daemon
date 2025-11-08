//! Status command implementation

use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;

use crate::client::HalpiClient;

/// Display status and measurement data from the device
pub async fn status() -> Result<()> {
    let client = HalpiClient::new();
    let values = client.get_values().await?;

    print_status_table(&values);

    Ok(())
}

/// Print status values in a formatted table
fn print_status_table(values: &HashMap<String, Value>) {
    println!();

    // Hardware/Firmware versions
    print_row(
        "hardware_version",
        &get_value_str(values, "hardware_version"),
        "",
    );
    print_row(
        "firmware_version",
        &get_value_str(values, "firmware_version"),
        "",
    );
    println!();

    // State and outputs
    print_row("state", &get_value_str(values, "state"), "");
    print_row(
        "5v_output_enabled",
        &get_value_str(values, "5v_output_enabled"),
        "",
    );

    // USB port states
    if let Some(usb_state) = values.get("usb_port_state").and_then(|v| v.as_u64()) {
        let usb_summary: Vec<String> = (0..4)
            .map(|i| {
                let enabled = (usb_state & (1 << i)) != 0;
                format!("USB{}:{}", i, if enabled { "✓" } else { "✗" })
            })
            .collect();
        print_row("usb_ports", &usb_summary.join(" "), "");
    }

    // Watchdog
    print_row(
        "watchdog_enabled",
        &get_value_str(values, "watchdog_enabled"),
        "",
    );
    if let Some(true) = values.get("watchdog_enabled").and_then(|v| v.as_bool()) {
        if let Some(timeout) = values.get("watchdog_timeout").and_then(|v| v.as_f64()) {
            print_row("watchdog_timeout", &format!("{:.1}", timeout), "s");
        }
        if let Some(elapsed) = values.get("watchdog_elapsed").and_then(|v| v.as_f64()) {
            print_row("watchdog_elapsed", &format!("{:.1}", elapsed), "s");
        }
    }
    println!();

    // Measurements
    if let Some(v_in) = values.get("V_in").and_then(|v| v.as_f64()) {
        print_row("V_in", &format!("{:.1}", v_in), "V");
    }
    if let Some(i_in) = values.get("I_in").and_then(|v| v.as_f64()) {
        print_row("I_in", &format!("{:.2}", i_in), "A");
    }
    if let Some(v_supercap) = values.get("V_supercap").and_then(|v| v.as_f64()) {
        print_row("V_supercap", &format!("{:.2}", v_supercap), "V");
    }

    // Temperatures (convert from Kelvin to Celsius)
    if let Some(t_mcu) = values.get("T_mcu").and_then(|v| v.as_f64()) {
        print_row("T_mcu", &format!("{:.1}", t_mcu - 273.15), "°C");
    }
    if let Some(t_pcb) = values.get("T_pcb").and_then(|v| v.as_f64()) {
        print_row("T_pcb", &format!("{:.1}", t_pcb - 273.15), "°C");
    }

    println!();
}

/// Print a formatted table row
fn print_row(key: &str, value: &str, unit: &str) {
    if unit.is_empty() {
        println!("{:<24} {:>15}", key, value);
    } else {
        println!("{:<24} {:>15} {}", key, value, unit);
    }
}

/// Helper to get a value as string, or "N/A" if not present
fn get_value_str(values: &HashMap<String, Value>, key: &str) -> String {
    values
        .get(key)
        .map(|v| match v {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => "null".to_string(),
            _ => v.to_string(),
        })
        .unwrap_or_else(|| "N/A".to_string())
}
