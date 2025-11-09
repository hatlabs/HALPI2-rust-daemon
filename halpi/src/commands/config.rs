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

/// Set a specific configuration value
pub async fn config_set(key: &str, value_str: &str) -> Result<()> {
    let client = HalpiClient::new();

    // Try to parse value as appropriate type
    let value = parse_value(value_str)?;

    client.set_config(key, value).await?;
    println!("Configuration '{}' set to: {}", key, value_str);

    Ok(())
}

/// Parse a string value into appropriate JSON type
fn parse_value(value_str: &str) -> Result<Value> {
    // Try parsing as boolean first
    if value_str.eq_ignore_ascii_case("true") {
        return Ok(Value::Bool(true));
    }
    if value_str.eq_ignore_ascii_case("false") {
        return Ok(Value::Bool(false));
    }

    // Try parsing as integer
    if let Ok(i) = value_str.parse::<i64>() {
        return Ok(Value::Number(i.into()));
    }

    // Try parsing as float
    if let Ok(f) = value_str.parse::<f64>() {
        if let Some(n) = serde_json::Number::from_f64(f) {
            return Ok(Value::Number(n));
        }
    }

    // Default to string
    Ok(Value::String(value_str.to_string()))
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
