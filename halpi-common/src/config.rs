//! Configuration types and loading for HALPI2 daemon

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Default configuration file location
pub const DEFAULT_CONFIG_FILE: &str = "/etc/halpid/halpid.conf";

/// Default I2C bus number (Raspberry Pi I2C bus 1)
pub const DEFAULT_I2C_BUS: u8 = 1;

/// Default I2C address for HALPI2 controller
pub const DEFAULT_I2C_ADDR: u8 = 0x6D;

/// Default blackout time limit in seconds
pub const DEFAULT_BLACKOUT_TIME_LIMIT: f64 = 5.0;

/// Default blackout voltage limit in volts
pub const DEFAULT_BLACKOUT_VOLTAGE_LIMIT: f64 = 9.0;

/// Default socket group name
pub const DEFAULT_SOCKET_GROUP: &str = "adm";

/// Default poweroff command
pub const DEFAULT_POWEROFF_COMMAND: &str = "/sbin/poweroff";

/// Configuration for the HALPI2 daemon
///
/// This struct holds all configuration options that can be set via:
/// - Configuration file (YAML)
/// - Command-line arguments
/// - Defaults
///
/// Field names with underscores map to dash-separated keys in YAML
/// (e.g., `i2c_bus` <-> `i2c-bus`)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Config {
    /// I2C bus number
    #[serde(default = "default_i2c_bus")]
    pub i2c_bus: u8,

    /// I2C device address (in hex, e.g., 0x6D)
    #[serde(default = "default_i2c_addr")]
    pub i2c_addr: u8,

    /// Blackout time limit in seconds
    ///
    /// Input voltage glitches shorter than this time will not trigger shutdown
    #[serde(default = "default_blackout_time_limit")]
    pub blackout_time_limit: f64,

    /// Blackout voltage limit in volts
    ///
    /// The device will initiate shutdown if input voltage drops below this value
    /// for the blackout time limit duration
    #[serde(default = "default_blackout_voltage_limit")]
    pub blackout_voltage_limit: f64,

    /// Path to UNIX socket for daemon communication
    ///
    /// If None, auto-determined based on user privileges:
    /// - root: `/var/run/halpid.sock`
    /// - non-root: `~/.halpid.sock`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub socket: Option<PathBuf>,

    /// Group name for UNIX socket permissions
    #[serde(default = "default_socket_group")]
    pub socket_group: String,

    /// Command to execute for system poweroff
    #[serde(default = "default_poweroff_command")]
    pub poweroff: String,
}

// Default value functions for serde
fn default_i2c_bus() -> u8 {
    DEFAULT_I2C_BUS
}

fn default_i2c_addr() -> u8 {
    DEFAULT_I2C_ADDR
}

fn default_blackout_time_limit() -> f64 {
    DEFAULT_BLACKOUT_TIME_LIMIT
}

fn default_blackout_voltage_limit() -> f64 {
    DEFAULT_BLACKOUT_VOLTAGE_LIMIT
}

fn default_socket_group() -> String {
    DEFAULT_SOCKET_GROUP.to_string()
}

fn default_poweroff_command() -> String {
    DEFAULT_POWEROFF_COMMAND.to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            i2c_bus: DEFAULT_I2C_BUS,
            i2c_addr: DEFAULT_I2C_ADDR,
            blackout_time_limit: DEFAULT_BLACKOUT_TIME_LIMIT,
            blackout_voltage_limit: DEFAULT_BLACKOUT_VOLTAGE_LIMIT,
            socket: None,
            socket_group: DEFAULT_SOCKET_GROUP.to_string(),
            poweroff: DEFAULT_POWEROFF_COMMAND.to_string(),
        }
    }
}

impl Config {
    /// Load configuration from a YAML file
    ///
    /// Returns `Ok(Config)` if the file exists and is valid YAML.
    /// Returns an error if the file exists but cannot be read or parsed.
    pub fn from_file(path: impl AsRef<std::path::Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let contents =
            std::fs::read_to_string(path).map_err(|e| ConfigError::FileRead(path.into(), e))?;

        serde_yaml::from_str(&contents)
            .map_err(|e| ConfigError::YamlParse(path.into(), e.to_string()))
    }

    /// Load configuration from a file if it exists, otherwise return defaults
    ///
    /// This is useful for the default config file location where a missing file is not an error.
    pub fn from_file_or_default(path: impl AsRef<std::path::Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        if path.exists() {
            Self::from_file(path)
        } else {
            Ok(Self::default())
        }
    }

    /// Validate configuration values
    ///
    /// Returns an error if any values are out of acceptable ranges
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate I2C bus (0-255, but realistically 0-10 on RPi)
        if self.i2c_bus > 10 {
            return Err(ConfigError::InvalidValue(format!(
                "i2c-bus {} is unusually high (expected 0-10)",
                self.i2c_bus
            )));
        }

        // Validate blackout time limit (must be positive, reasonable upper bound)
        if self.blackout_time_limit <= 0.0 {
            return Err(ConfigError::InvalidValue(
                "blackout_time_limit must be positive".to_string(),
            ));
        }
        if self.blackout_time_limit > 3600.0 {
            return Err(ConfigError::InvalidValue(
                "blackout_time_limit must be <= 3600 seconds (1 hour)".to_string(),
            ));
        }

        // Validate blackout voltage limit (typical range: 5-15V)
        if self.blackout_voltage_limit < 5.0 || self.blackout_voltage_limit > 15.0 {
            return Err(ConfigError::InvalidValue(format!(
                "blackout-voltage-limit {} is out of range (expected 5.0-15.0 volts)",
                self.blackout_voltage_limit
            )));
        }

        Ok(())
    }

    /// Merge another Config into this one, overriding fields that are explicitly set
    ///
    /// This is used to implement the precedence: CLI > file > defaults
    pub fn merge(&mut self, other: Config) {
        self.i2c_bus = other.i2c_bus;
        self.i2c_addr = other.i2c_addr;
        self.blackout_time_limit = other.blackout_time_limit;
        self.blackout_voltage_limit = other.blackout_voltage_limit;

        // Only override if explicitly set in other
        if other.socket.is_some() {
            self.socket = other.socket;
        }

        if other.socket_group != DEFAULT_SOCKET_GROUP {
            self.socket_group = other.socket_group;
        }

        if other.poweroff != DEFAULT_POWEROFF_COMMAND {
            self.poweroff = other.poweroff;
        }
    }
}

/// Configuration loading errors
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Failed to read config file {0}: {1}")]
    FileRead(PathBuf, #[source] std::io::Error),

    #[error("Failed to parse YAML config file {0}: {1}")]
    YamlParse(PathBuf, String),

    #[error("Invalid configuration value: {0}")]
    InvalidValue(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.i2c_bus, 1);
        assert_eq!(config.i2c_addr, 0x6D);
        assert_eq!(config.blackout_time_limit, 5.0);
        assert_eq!(config.blackout_voltage_limit, 9.0);
        assert_eq!(config.socket, None);
        assert_eq!(config.socket_group, "adm");
        assert_eq!(config.poweroff, "/sbin/poweroff");
    }

    #[test]
    fn test_validate_valid_config() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_invalid_blackout_time() {
        let config = Config {
            blackout_time_limit: -1.0,
            ..Default::default()
        };
        assert!(config.validate().is_err());

        let config = Config {
            blackout_time_limit: 5000.0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_invalid_blackout_voltage() {
        let config = Config {
            blackout_voltage_limit: 3.0,
            ..Default::default()
        };
        assert!(config.validate().is_err());

        let config = Config {
            blackout_voltage_limit: 20.0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_yaml_deserialization_with_dashes() {
        let yaml = r#"
i2c-bus: 2
i2c-addr: 0x6E
blackout-time-limit: 10.0
blackout-voltage-limit: 8.5
socket-group: users
poweroff: /usr/bin/poweroff
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.i2c_bus, 2);
        assert_eq!(config.i2c_addr, 0x6E);
        assert_eq!(config.blackout_time_limit, 10.0);
        assert_eq!(config.blackout_voltage_limit, 8.5);
        assert_eq!(config.socket_group, "users");
        assert_eq!(config.poweroff, "/usr/bin/poweroff");
    }

    #[test]
    fn test_yaml_deserialization_partial() {
        let yaml = r#"
blackout-time-limit: 15.0
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.i2c_bus, 1); // default
        assert_eq!(config.blackout_time_limit, 15.0); // overridden
    }

    #[test]
    fn test_config_merge() {
        let mut base = Config::default();
        let override_config = Config {
            i2c_bus: 3,
            blackout_time_limit: 20.0,
            ..Default::default()
        };

        base.merge(override_config);

        assert_eq!(base.i2c_bus, 3);
        assert_eq!(base.blackout_time_limit, 20.0);
        assert_eq!(base.socket_group, "adm"); // unchanged
    }
}
