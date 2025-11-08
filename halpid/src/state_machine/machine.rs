//! State machine implementation for power management

#[cfg(target_os = "linux")]
use std::sync::Arc;
#[cfg(target_os = "linux")]
use std::time::Instant;
#[cfg(target_os = "linux")]
use tokio::sync::{Mutex, RwLock};
#[cfg(target_os = "linux")]
use tokio::time::{Duration, interval};
#[cfg(target_os = "linux")]
use tracing::{error, info, warn};

#[cfg(target_os = "linux")]
use halpi_common::config::Config;

#[cfg(target_os = "linux")]
use crate::i2c::HalpiDevice;

/// Daemon state machine states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonState {
    /// Initial state - initializing watchdog
    Start,
    /// Normal operation - monitoring for blackout
    Ok,
    /// Blackout detected - waiting for power restoration or timeout
    Blackout,
    /// Shutdown sequence initiated
    Shutdown,
    /// Waiting for power loss after shutdown
    Dead,
}

/// Power management state machine
#[cfg(target_os = "linux")]
pub struct StateMachine {
    state: DaemonState,
    device: Arc<Mutex<HalpiDevice>>,
    config: Arc<RwLock<Config>>,
    blackout_start: Option<Instant>,
}

#[cfg(target_os = "linux")]
impl StateMachine {
    /// Create a new state machine
    pub fn new(device: Arc<Mutex<HalpiDevice>>, config: Arc<RwLock<Config>>) -> Self {
        Self {
            state: DaemonState::Start,
            device,
            config,
            blackout_start: None,
        }
    }

    /// Get current state
    pub fn state(&self) -> DaemonState {
        self.state
    }

    /// Run the state machine loop
    ///
    /// CRITICAL: Polls every 0.1 seconds (100ms) for responsive power management
    pub async fn run(&mut self) {
        info!("Starting power management state machine");

        // Critical timing: 0.1 second polling interval
        let mut ticker = interval(Duration::from_millis(100));

        loop {
            ticker.tick().await;

            if let Err(e) = self.tick().await {
                error!("State machine error: {}", e);
            }
        }
    }

    /// Execute one state machine iteration
    async fn tick(&mut self) -> anyhow::Result<()> {
        let config = self.config.read().await;

        match self.state {
            DaemonState::Start => {
                info!("Initializing watchdog");
                let mut device = self.device.lock().await;
                device.set_watchdog_timeout(10000)?;
                drop(device);

                self.transition_to(DaemonState::Ok);
            }

            DaemonState::Ok => {
                // Read DC input voltage
                let v_in = {
                    let mut device = self.device.lock().await;
                    device.get_dcin_voltage()?
                };

                // Check for blackout
                if v_in < config.blackout_voltage_limit {
                    warn!(
                        "Detected blackout (V_in = {:.2}V < {:.2}V)",
                        v_in, config.blackout_voltage_limit
                    );
                    self.blackout_start = Some(Instant::now());
                    self.transition_to(DaemonState::Blackout);
                }

                // Feed watchdog in normal operation
                let mut device = self.device.lock().await;
                device.feed_watchdog()?;
            }

            DaemonState::Blackout => {
                // Read DC input voltage
                let v_in = {
                    let mut device = self.device.lock().await;
                    device.get_dcin_voltage()?
                };

                // Check for power restoration
                if v_in > config.blackout_voltage_limit {
                    info!("Power resumed (V_in = {:.2}V)", v_in);
                    self.blackout_start = None;
                    self.transition_to(DaemonState::Ok);
                } else if let Some(start) = self.blackout_start {
                    // Check timeout
                    let elapsed = start.elapsed().as_secs_f64();
                    if elapsed > config.blackout_time_limit {
                        warn!("Blacked out for {:.1}s, initiating shutdown", elapsed);
                        self.transition_to(DaemonState::Shutdown);
                    }
                }

                // Continue feeding watchdog during blackout
                let mut device = self.device.lock().await;
                device.feed_watchdog()?;
            }

            DaemonState::Shutdown => {
                // Notify device of shutdown
                let mut device = self.device.lock().await;
                device.request_shutdown()?;
                drop(device);

                // Execute poweroff command
                if !config.poweroff.is_empty() {
                    info!("Executing: {}", config.poweroff);
                    std::process::Command::new("sudo")
                        .arg(&config.poweroff)
                        .spawn()?;
                } else {
                    warn!("Dry-run mode: poweroff command is empty");
                }

                self.transition_to(DaemonState::Dead);
            }

            DaemonState::Dead => {
                // Just wait for the inevitable power loss
                // No watchdog feeding - let it timeout and cut power
            }
        }

        Ok(())
    }

    /// Transition to a new state with logging
    fn transition_to(&mut self, new_state: DaemonState) {
        info!("State transition: {:?} -> {:?}", self.state, new_state);
        self.state = new_state;
    }
}
