//! Core data types for HALPI2 daemon and CLI
//!
//! This module provides shared types used across the daemon, CLI, and API:
//! - Version: Hardware and firmware version information
//! - Measurements: Combined sensor readings from the device
//! - PowerState: Current power management state

use serde::{Deserialize, Serialize};
use std::fmt;

/// Version information for hardware or firmware
///
/// Format: major.minor.patch[-alpha]
/// Alpha byte 0xFF (255) indicates a release version (no alpha suffix)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
    /// Alpha version number, or 255 for release versions
    pub alpha: u8,
}

impl Version {
    /// Create a new version from raw bytes (as read from I2C)
    pub fn from_bytes(bytes: [u8; 4]) -> Self {
        Self {
            major: bytes[0],
            minor: bytes[1],
            patch: bytes[2],
            alpha: bytes[3],
        }
    }

    /// Create a release version (no alpha)
    pub fn new(major: u8, minor: u8, patch: u8) -> Self {
        Self {
            major,
            minor,
            patch,
            alpha: 255,
        }
    }

    /// Create an alpha version
    pub fn new_alpha(major: u8, minor: u8, patch: u8, alpha: u8) -> Self {
        Self {
            major,
            minor,
            patch,
            alpha,
        }
    }

    /// Check if this is a release version (no alpha)
    pub fn is_release(&self) -> bool {
        self.alpha == 255
    }

    /// Check if this is an unavailable version (major = 0xFF)
    pub fn is_unavailable(&self) -> bool {
        self.major == 255
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_unavailable() {
            write!(f, "N/A")
        } else if self.is_release() {
            write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
        } else {
            write!(
                f,
                "{}.{}.{}-a{}",
                self.major, self.minor, self.patch, self.alpha
            )
        }
    }
}

/// Power management state
///
/// These values must match the HALPI2 firmware state machine exactly
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum PowerState {
    PowerOff = 0,
    OffCharging = 1,
    SystemStartup = 2,
    OperationalSolo = 3,
    OperationalCoOp = 4,
    BlackoutSolo = 5,
    BlackoutCoOp = 6,
    BlackoutShutdown = 7,
    ManualShutdown = 8,
    PoweredDownBlackout = 9,
    PoweredDownManual = 10,
    HostUnresponsive = 11,
    EnteringStandby = 12,
    Standby = 13,
}

impl PowerState {
    /// Create a PowerState from a raw byte value
    pub fn from_byte(value: u8) -> Option<Self> {
        match value {
            0 => Some(PowerState::PowerOff),
            1 => Some(PowerState::OffCharging),
            2 => Some(PowerState::SystemStartup),
            3 => Some(PowerState::OperationalSolo),
            4 => Some(PowerState::OperationalCoOp),
            5 => Some(PowerState::BlackoutSolo),
            6 => Some(PowerState::BlackoutCoOp),
            7 => Some(PowerState::BlackoutShutdown),
            8 => Some(PowerState::ManualShutdown),
            9 => Some(PowerState::PoweredDownBlackout),
            10 => Some(PowerState::PoweredDownManual),
            11 => Some(PowerState::HostUnresponsive),
            12 => Some(PowerState::EnteringStandby),
            13 => Some(PowerState::Standby),
            _ => None,
        }
    }

    /// Get the state name as a string
    pub fn name(&self) -> &'static str {
        match self {
            PowerState::PowerOff => "PowerOff",
            PowerState::OffCharging => "OffCharging",
            PowerState::SystemStartup => "SystemStartup",
            PowerState::OperationalSolo => "OperationalSolo",
            PowerState::OperationalCoOp => "OperationalCoOp",
            PowerState::BlackoutSolo => "BlackoutSolo",
            PowerState::BlackoutCoOp => "BlackoutCoOp",
            PowerState::BlackoutShutdown => "BlackoutShutdown",
            PowerState::ManualShutdown => "ManualShutdown",
            PowerState::PoweredDownBlackout => "PoweredDownBlackout",
            PowerState::PoweredDownManual => "PoweredDownManual",
            PowerState::HostUnresponsive => "HostUnresponsive",
            PowerState::EnteringStandby => "EnteringStandby",
            PowerState::Standby => "Standby",
        }
    }
}

impl fmt::Display for PowerState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Combined sensor measurements from the HALPI2 device
///
/// All temperature values are stored in Kelvin internally but can be
/// converted to Celsius for display using the helper methods.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Measurements {
    /// DC input voltage (V)
    pub dcin_voltage: f32,
    /// Supercapacitor voltage (V)
    pub supercap_voltage: f32,
    /// Input current (A)
    pub input_current: f32,
    /// MCU temperature (Kelvin)
    pub mcu_temperature: f32,
    /// PCB temperature (Kelvin)
    pub pcb_temperature: f32,
    /// Current power state
    pub power_state: PowerState,
    /// Watchdog elapsed time (seconds)
    pub watchdog_elapsed: f32,
}

impl Measurements {
    /// Convert MCU temperature from Kelvin to Celsius
    pub fn mcu_temperature_celsius(&self) -> f32 {
        self.mcu_temperature - 273.15
    }

    /// Convert PCB temperature from Kelvin to Celsius
    pub fn pcb_temperature_celsius(&self) -> f32 {
        self.pcb_temperature - 273.15
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_release() {
        let version = Version::new(3, 1, 2);
        assert_eq!(version.to_string(), "3.1.2");
        assert!(version.is_release());
        assert!(!version.is_unavailable());
    }

    #[test]
    fn test_version_alpha() {
        let version = Version::new_alpha(3, 1, 2, 5);
        assert_eq!(version.to_string(), "3.1.2-a5");
        assert!(!version.is_release());
        assert!(!version.is_unavailable());
    }

    #[test]
    fn test_version_from_bytes() {
        let bytes = [3, 1, 2, 255];
        let version = Version::from_bytes(bytes);
        assert_eq!(version.to_string(), "3.1.2");

        let bytes_alpha = [3, 1, 2, 5];
        let version_alpha = Version::from_bytes(bytes_alpha);
        assert_eq!(version_alpha.to_string(), "3.1.2-a5");
    }

    #[test]
    fn test_version_unavailable() {
        let version = Version::from_bytes([255, 0, 0, 0]);
        assert_eq!(version.to_string(), "N/A");
        assert!(version.is_unavailable());
    }

    #[test]
    fn test_power_state_from_byte() {
        assert_eq!(
            PowerState::from_byte(0),
            Some(PowerState::PowerOff)
        );
        assert_eq!(
            PowerState::from_byte(3),
            Some(PowerState::OperationalSolo)
        );
        assert_eq!(
            PowerState::from_byte(13),
            Some(PowerState::Standby)
        );
        assert_eq!(PowerState::from_byte(99), None);
    }

    #[test]
    fn test_power_state_name() {
        assert_eq!(PowerState::PowerOff.name(), "PowerOff");
        assert_eq!(PowerState::OperationalSolo.name(), "OperationalSolo");
        assert_eq!(PowerState::Standby.name(), "Standby");
    }

    #[test]
    fn test_power_state_display() {
        assert_eq!(PowerState::PowerOff.to_string(), "PowerOff");
        assert_eq!(PowerState::OperationalCoOp.to_string(), "OperationalCoOp");
    }

    #[test]
    fn test_measurements_temperature_conversion() {
        let measurements = Measurements {
            dcin_voltage: 12.5,
            supercap_voltage: 10.2,
            input_current: 1.5,
            mcu_temperature: 298.15, // 25°C in Kelvin
            pcb_temperature: 303.15, // 30°C in Kelvin
            power_state: PowerState::OperationalSolo,
            watchdog_elapsed: 2.5,
        };

        assert!((measurements.mcu_temperature_celsius() - 25.0).abs() < 0.01);
        assert!((measurements.pcb_temperature_celsius() - 30.0).abs() < 0.01);
    }

    #[test]
    fn test_version_json_serialization() {
        let version = Version::new_alpha(3, 1, 2, 5);
        let json = serde_json::to_string(&version).unwrap();
        let deserialized: Version = serde_json::from_str(&json).unwrap();
        assert_eq!(version, deserialized);
    }

    #[test]
    fn test_power_state_json_serialization() {
        let state = PowerState::OperationalSolo;
        let json = serde_json::to_string(&state).unwrap();
        assert_eq!(json, "\"OperationalSolo\"");
        let deserialized: PowerState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, deserialized);
    }

    #[test]
    fn test_measurements_json_serialization() {
        let measurements = Measurements {
            dcin_voltage: 12.5,
            supercap_voltage: 10.2,
            input_current: 1.5,
            mcu_temperature: 298.15,
            pcb_temperature: 303.15,
            power_state: PowerState::OperationalSolo,
            watchdog_elapsed: 2.5,
        };

        let json = serde_json::to_string(&measurements).unwrap();
        let deserialized: Measurements = serde_json::from_str(&json).unwrap();
        assert_eq!(measurements.dcin_voltage, deserialized.dcin_voltage);
        assert_eq!(measurements.power_state, deserialized.power_state);
    }
}
