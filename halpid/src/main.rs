pub mod daemon;
pub mod i2c;
pub mod server;
pub mod state_machine;

use clap::Parser;
use std::path::PathBuf;
#[cfg(target_os = "linux")]
use std::sync::Arc;
#[cfg(target_os = "linux")]
use tokio::sync::{Mutex, RwLock};
#[cfg(target_os = "linux")]
use tracing::{error, info};
#[cfg(target_os = "linux")]
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[cfg(target_os = "linux")]
use halpi_common::config::Config;

#[cfg(target_os = "linux")]
use i2c::HalpiDevice;
#[cfg(target_os = "linux")]
use server::app::AppState;
#[cfg(target_os = "linux")]
use state_machine::StateMachine;

/// HALPI2 power monitor and watchdog daemon
#[derive(Parser)]
#[command(name = "halpid")]
#[command(about = "HALPI2 power monitor and watchdog daemon", long_about = None)]
#[command(version)]
struct Cli {
    /// Path to configuration file
    #[arg(short, long, value_name = "FILE")]
    conf: Option<PathBuf>,

    /// I2C bus number
    #[arg(long)]
    i2c_bus: Option<u8>,

    /// I2C device address (hex)
    #[arg(long, value_parser = clap::value_parser!(u8))]
    i2c_addr: Option<u8>,

    /// Unix socket path
    #[arg(long)]
    socket: Option<PathBuf>,

    /// Blackout time limit (seconds)
    #[arg(long)]
    blackout_time_limit: Option<f64>,

    /// Blackout voltage limit (volts)
    #[arg(long)]
    blackout_voltage_limit: Option<f64>,

    /// Poweroff command (empty string for dry-run)
    #[arg(long)]
    poweroff: Option<String>,
}

#[cfg(target_os = "linux")]
#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "halpid=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("halpid - HALPI2 power monitor and watchdog daemon");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));

    let cli = Cli::parse();

    // Load configuration
    let mut config = if let Some(conf_path) = cli.conf {
        match Config::from_file(&conf_path) {
            Ok(c) => {
                info!("Loaded configuration from {}", conf_path.display());
                c
            }
            Err(e) => {
                error!("Failed to load configuration: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        Config::default()
    };

    // Apply CLI overrides
    if let Some(i2c_bus) = cli.i2c_bus {
        config.i2c_bus = i2c_bus;
    }
    if let Some(i2c_addr) = cli.i2c_addr {
        config.i2c_addr = i2c_addr;
    }
    if let Some(socket) = cli.socket {
        config.socket = Some(socket);
    }
    if let Some(blackout_time_limit) = cli.blackout_time_limit {
        config.blackout_time_limit = blackout_time_limit;
    }
    if let Some(blackout_voltage_limit) = cli.blackout_voltage_limit {
        config.blackout_voltage_limit = blackout_voltage_limit;
    }
    if let Some(poweroff) = cli.poweroff {
        config.poweroff = poweroff;
    }

    info!(
        "Configuration: I2C bus {}, address 0x{:02X}",
        config.i2c_bus, config.i2c_addr
    );

    // Open I2C device
    let device = match HalpiDevice::new(config.i2c_bus, config.i2c_addr) {
        Ok(dev) => {
            info!("Opened I2C device");
            Arc::new(Mutex::new(dev))
        }
        Err(e) => {
            error!("Failed to open I2C device: {}", e);
            std::process::exit(1);
        }
    };

    let config_arc = Arc::new(RwLock::new(config.clone()));

    // Create shared state for HTTP server
    let app_state = AppState::new(device.clone(), config_arc.clone());

    // Get socket path for cleanup
    let socket_path = config
        .socket
        .clone()
        .unwrap_or_else(|| PathBuf::from("/run/halpid/halpid.sock"));

    // Spawn concurrent tasks
    let server_handle = {
        let app_state = app_state.clone();
        tokio::spawn(async move {
            info!("Starting HTTP server");
            if let Err(e) = server::app::run_server(app_state).await {
                error!("Server error: {}", e);
            }
        })
    };

    let state_machine_handle = {
        let device = device.clone();
        let config = config_arc.clone();
        tokio::spawn(async move {
            info!("Starting state machine");
            let mut sm = StateMachine::new(device, config);
            sm.run().await;
        })
    };

    let signal_handle = tokio::spawn(async move {
        daemon::wait_for_signal().await;
    });

    // Wait for any task to complete (signal will finish first on shutdown)
    tokio::select! {
        _ = server_handle => {
            info!("Server task completed");
        }
        _ = state_machine_handle => {
            info!("State machine task completed");
        }
        _ = signal_handle => {
            info!("Signal received, initiating shutdown");
        }
    }

    // Run cleanup
    daemon::signals::cleanup(device, &socket_path).await;

    info!("Daemon shutdown complete");
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("halpid requires Linux for I2C device access");
    std::process::exit(1);
}
