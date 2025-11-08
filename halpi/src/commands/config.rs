//! Configuration command implementation

use anyhow::Result;
use serde_json::Value;

use crate::client::HalpiClient;

/// Display all configuration values
pub async fn config_get_all() -> Result<()> {
    let client = HalpiClient::new();
    let config = client.get_config().await?;

    println!();
    for (key, value) in &config {
        let formatted_value = format_value(value);
        println!("{:<30} {:>15}", key, formatted_value);
    }
    println!();

    Ok(())
}

/// Display a specific configuration value
pub async fn config_get(key: &str) -> Result<()> {
    let client = HalpiClient::new();
    let config = client.get_config().await?;

    match config.get(key) {
        Some(value) => {
            println!("{}", format_value(value));
            Ok(())
        }
        None => {
            anyhow::bail!("Configuration key '{}' not found", key);
        }
    }
}

/// Format a JSON value for display
fn format_value(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                // Format floats with up to 2 decimal places, strip trailing zeros
                format!("{:.2}", f)
                    .trim_end_matches('0')
                    .trim_end_matches('.')
                    .to_string()
            } else {
                n.to_string()
            }
        }
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        _ => value.to_string(),
    }
}
