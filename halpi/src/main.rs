mod client;
mod commands;

use clap::{Parser, Subcommand};

/// HALPI2 command-line interface
#[derive(Parser)]
#[command(name = "halpi")]
#[command(about = "HALPI2 command-line interface", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Display status and measurement data from the device
    Status,
    /// Display version information
    Version,
    /// Get or set configuration values
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },
    /// Shutdown or standby the system
    Shutdown {
        /// Enter standby mode instead of shutdown
        #[arg(long, requires = "time")]
        standby: bool,
        /// Wakeup time for standby (seconds or datetime string)
        #[arg(long)]
        time: Option<String>,
    },
    /// Control USB port power
    Usb {
        #[command(subcommand)]
        action: Option<UsbAction>,
    },
    /// Upload firmware to the device
    Flash {
        /// Path to firmware binary file
        firmware: String,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Get a configuration value
    Get {
        /// Configuration key to get
        key: String,
    },
    /// Set a configuration value
    Set {
        /// Configuration key to set
        key: String,
        /// Value to set
        value: String,
    },
}

#[derive(Subcommand)]
enum UsbAction {
    /// Enable a USB port (0-3 or 'all')
    Enable {
        /// Port number (0-3) or 'all'
        port: String,
    },
    /// Disable a USB port (0-3 or 'all')
    Disable {
        /// Port number (0-3) or 'all'
        port: String,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Some(Commands::Status) => commands::status::status().await,
        Some(Commands::Version) | None => {
            println!("halpi version {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some(Commands::Config { action }) => match action {
            Some(ConfigAction::Get { key }) => commands::config::config_get(&key).await,
            Some(ConfigAction::Set { key, value }) => {
                commands::config::config_set(&key, &value).await
            }
            None => commands::config::config_get_all().await,
        },
        Some(Commands::Shutdown { standby, time }) => {
            if standby {
                // Clap enforces that time is present when standby is true (via requires attribute)
                let t = time.unwrap();
                // Try to parse as integer (seconds), otherwise treat as datetime
                if let Ok(delay) = t.parse::<u32>() {
                    commands::shutdown::standby_delay(delay).await
                } else {
                    commands::shutdown::standby_datetime(&t).await
                }
            } else {
                commands::shutdown::shutdown().await
            }
        }
        Some(Commands::Usb { action }) => match action {
            Some(UsbAction::Enable { port }) => commands::usb::usb_enable(&port).await,
            Some(UsbAction::Disable { port }) => commands::usb::usb_disable(&port).await,
            None => commands::usb::usb_status().await,
        },
        Some(Commands::Flash { firmware }) => commands::flash::flash(&firmware).await,
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn test_cli_verify() {
        // Verify CLI structure is valid
        Cli::command().debug_assert();
    }

    #[test]
    fn test_cli_status_command() {
        let cli = Cli::try_parse_from(["halpi", "status"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Status)));
    }

    #[test]
    fn test_cli_version_command() {
        let cli = Cli::try_parse_from(["halpi", "version"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Version)));
    }

    #[test]
    fn test_cli_config_all() {
        let cli = Cli::try_parse_from(["halpi", "config"]).unwrap();
        match cli.command {
            Some(Commands::Config { action }) => assert!(action.is_none()),
            _ => panic!("Expected Config command"),
        }
    }

    #[test]
    fn test_cli_config_get() {
        let cli = Cli::try_parse_from(["halpi", "config", "get", "watchdog_timeout"]).unwrap();
        match cli.command {
            Some(Commands::Config { action }) => match action {
                Some(ConfigAction::Get { key }) => {
                    assert_eq!(key, "watchdog_timeout");
                }
                _ => panic!("Expected Get action"),
            },
            _ => panic!("Expected Config command"),
        }
    }

    #[test]
    fn test_cli_config_set() {
        let cli = Cli::try_parse_from(["halpi", "config", "set", "led_brightness", "100"]).unwrap();
        match cli.command {
            Some(Commands::Config { action }) => match action {
                Some(ConfigAction::Set { key, value }) => {
                    assert_eq!(key, "led_brightness");
                    assert_eq!(value, "100");
                }
                _ => panic!("Expected Set action"),
            },
            _ => panic!("Expected Config command"),
        }
    }

    #[test]
    fn test_cli_shutdown() {
        let cli = Cli::try_parse_from(["halpi", "shutdown"]).unwrap();
        match cli.command {
            Some(Commands::Shutdown { standby, time }) => {
                assert!(!standby);
                assert!(time.is_none());
            }
            _ => panic!("Expected Shutdown command"),
        }
    }

    #[test]
    fn test_cli_standby_with_delay() {
        let cli = Cli::try_parse_from(["halpi", "shutdown", "--standby", "--time", "300"]).unwrap();
        match cli.command {
            Some(Commands::Shutdown { standby, time }) => {
                assert!(standby);
                assert_eq!(time, Some("300".to_string()));
            }
            _ => panic!("Expected Shutdown command"),
        }
    }

    #[test]
    fn test_cli_standby_with_datetime() {
        let cli = Cli::try_parse_from([
            "halpi",
            "shutdown",
            "--standby",
            "--time",
            "2025-12-31T23:59:59",
        ])
        .unwrap();
        match cli.command {
            Some(Commands::Shutdown { standby, time }) => {
                assert!(standby);
                assert_eq!(time, Some("2025-12-31T23:59:59".to_string()));
            }
            _ => panic!("Expected Shutdown command"),
        }
    }

    #[test]
    fn test_cli_usb_status() {
        let cli = Cli::try_parse_from(["halpi", "usb"]).unwrap();
        match cli.command {
            Some(Commands::Usb { action }) => assert!(action.is_none()),
            _ => panic!("Expected Usb command"),
        }
    }

    #[test]
    fn test_cli_usb_enable() {
        let cli = Cli::try_parse_from(["halpi", "usb", "enable", "0"]).unwrap();
        match cli.command {
            Some(Commands::Usb { action }) => match action {
                Some(UsbAction::Enable { port }) => assert_eq!(port, "0"),
                _ => panic!("Expected Enable action"),
            },
            _ => panic!("Expected Usb command"),
        }
    }

    #[test]
    fn test_cli_usb_disable() {
        let cli = Cli::try_parse_from(["halpi", "usb", "disable", "all"]).unwrap();
        match cli.command {
            Some(Commands::Usb { action }) => match action {
                Some(UsbAction::Disable { port }) => assert_eq!(port, "all"),
                _ => panic!("Expected Disable action"),
            },
            _ => panic!("Expected Usb command"),
        }
    }

    #[test]
    fn test_cli_flash() {
        let cli = Cli::try_parse_from(["halpi", "flash", "/path/to/firmware.bin"]).unwrap();
        match cli.command {
            Some(Commands::Flash { firmware }) => assert_eq!(firmware, "/path/to/firmware.bin"),
            _ => panic!("Expected Flash command"),
        }
    }

    #[test]
    fn test_cli_standby_requires_time() {
        // This should fail because --standby requires --time
        let result = Cli::try_parse_from(["halpi", "shutdown", "--standby"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_cli_no_command_defaults_to_version() {
        let cli = Cli::try_parse_from(["halpi"]).unwrap();
        assert!(cli.command.is_none());
    }
}
