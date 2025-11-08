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
    /// Get configuration values
    Config {
        /// Configuration key to get
        key: Option<String>,
    },
    /// Shutdown or standby the system
    Shutdown {
        /// Enter standby mode instead of shutdown
        #[arg(long)]
        standby: bool,
        /// Wakeup time for standby (seconds or datetime string)
        #[arg(long, requires = "standby")]
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
        Some(Commands::Config { key }) => {
            if let Some(k) = key {
                commands::config::config_get(&k).await
            } else {
                commands::config::config_get_all().await
            }
        }
        Some(Commands::Shutdown { standby, time }) => {
            if standby {
                if let Some(t) = time {
                    // Try to parse as integer (seconds), otherwise treat as datetime
                    if let Ok(delay) = t.parse::<u32>() {
                        commands::shutdown::standby_delay(delay).await
                    } else {
                        commands::shutdown::standby_datetime(&t).await
                    }
                } else {
                    eprintln!("Error: --time is required when using --standby");
                    std::process::exit(1);
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
