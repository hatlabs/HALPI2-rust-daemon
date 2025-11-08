//! I2C protocol definitions for HALPI2 communication
//!
//! This module defines the I2C register addresses, data encoding/decoding,
//! and state enums for communicating with the HALPI2 RP2040 firmware.

use serde::{Deserialize, Serialize};

/// Flash block size for firmware updates (4 KiB)
pub const FLASH_BLOCK_SIZE: usize = 4096;

// ============================================================================
// Register Addresses
// ============================================================================

/// Hardware version (4 bytes: major.minor.patch-alpha)
pub const REG_HARDWARE_VERSION: u8 = 0x03;

/// Firmware version (4 bytes: major.minor.patch-alpha)
pub const REG_FIRMWARE_VERSION: u8 = 0x04;

/// Power control/status register
pub const REG_POWER_CONTROL: u8 = 0x10;

/// Watchdog timeout (word, milliseconds)
pub const REG_WATCHDOG_TIMEOUT: u8 = 0x12;

/// Power-on threshold (word, analog scaled)
pub const REG_POWER_ON_THRESHOLD: u8 = 0x13;

/// Solo power-off threshold (word, analog scaled)
pub const REG_SOLO_POWEROFF_THRESHOLD: u8 = 0x14;

/// Current power state (byte, PowerState enum)
pub const REG_POWER_STATE: u8 = 0x15;

/// Watchdog elapsed time (byte, 0.1s units)
pub const REG_WATCHDOG_ELAPSED: u8 = 0x16;

/// LED brightness (byte, 0-255)
pub const REG_LED_BRIGHTNESS: u8 = 0x17;

/// Auto-restart enable flag (byte, boolean)
pub const REG_AUTO_RESTART: u8 = 0x18;

/// Solo depleting timeout (4 bytes, big-endian u32, milliseconds)
pub const REG_SOLO_DEPLETING_TIMEOUT: u8 = 0x19;

/// DC input voltage (word, analog scaled)
pub const REG_DCIN_VOLTAGE: u8 = 0x20;

/// Supercapacitor voltage (word, analog scaled)
pub const REG_SUPERCAP_VOLTAGE: u8 = 0x21;

/// Input current (word, analog scaled)
pub const REG_INPUT_CURRENT: u8 = 0x22;

/// MCU temperature (word, analog scaled, Kelvin)
pub const REG_MCU_TEMPERATURE: u8 = 0x23;

/// PCB temperature (word, analog scaled, Kelvin)
pub const REG_PCB_TEMPERATURE: u8 = 0x24;

/// Request shutdown command (write byte)
pub const REG_REQUEST_SHUTDOWN: u8 = 0x30;

/// Request standby command (write byte)
pub const REG_REQUEST_STANDBY: u8 = 0x31;

/// Start firmware update (write 4 bytes: big-endian u32 total size)
pub const REG_DFU_START: u8 = 0x40;

/// Upload firmware block (write block_num + crc + data)
pub const REG_DFU_UPLOAD_BLOCK: u8 = 0x41;

/// Commit firmware update
pub const REG_DFU_COMMIT: u8 = 0x42;

/// Abort firmware update
pub const REG_DFU_ABORT: u8 = 0x43;

/// Get DFU status
pub const REG_DFU_STATUS: u8 = 0x44;

/// Get DFU error details
pub const REG_DFU_ERROR: u8 = 0x45;

// ============================================================================
// Power State Enum
// ============================================================================

/// Power management state machine states
///
/// These values must match the firmware state machine exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum PowerState {
    /// System is powered off, no power available
    PowerOff = 0,
    /// System is off but charging supercapacitor
    OffCharging = 1,
    /// System is starting up
    SystemStartup = 2,
    /// Operational, running on external power only
    OperationalSolo = 3,
    /// Operational, running on external power + supercap
    OperationalCoOp = 4,
    /// Blackout detected, running on supercap only
    BlackoutSolo = 5,
    /// Blackout detected, running on supercap in cooperative mode
    BlackoutCoOp = 6,
    /// Shutdown initiated due to blackout
    BlackoutShutdown = 7,
    /// Shutdown initiated manually
    ManualShutdown = 8,
    /// Powered down after blackout
    PoweredDownBlackout = 9,
    /// Powered down after manual shutdown
    PoweredDownManual = 10,
    /// Host is unresponsive (watchdog timeout)
    HostUnresponsive = 11,
    /// Entering standby mode
    EnteringStandby = 12,
    /// In standby mode (RTC wake)
    Standby = 13,
}

impl PowerState {
    /// Create PowerState from a byte value
    pub fn from_byte(value: u8) -> Result<Self, ProtocolError> {
        match value {
            0 => Ok(PowerState::PowerOff),
            1 => Ok(PowerState::OffCharging),
            2 => Ok(PowerState::SystemStartup),
            3 => Ok(PowerState::OperationalSolo),
            4 => Ok(PowerState::OperationalCoOp),
            5 => Ok(PowerState::BlackoutSolo),
            6 => Ok(PowerState::BlackoutCoOp),
            7 => Ok(PowerState::BlackoutShutdown),
            8 => Ok(PowerState::ManualShutdown),
            9 => Ok(PowerState::PoweredDownBlackout),
            10 => Ok(PowerState::PoweredDownManual),
            11 => Ok(PowerState::HostUnresponsive),
            12 => Ok(PowerState::EnteringStandby),
            13 => Ok(PowerState::Standby),
            _ => Err(ProtocolError::InvalidPowerState(value)),
        }
    }

    /// Convert PowerState to byte value
    pub fn to_byte(self) -> u8 {
        self as u8
    }

    /// Get human-readable name of the state
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

// ============================================================================
// DFU State Enum
// ============================================================================

/// Firmware update (DFU) state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum DFUState {
    /// Idle, ready for firmware update
    Idle = 0,
    /// Preparing for update
    Preparing = 1,
    /// Update in progress
    Updating = 2,
    /// Queue is full (too many blocks pending)
    QueueFull = 3,
    /// Ready to commit the update
    ReadyToCommit = 4,
    /// CRC error detected
    CrcError = 5,
    /// Data length error
    DataLengthError = 6,
    /// Flash write error
    WriteError = 7,
    /// Protocol error
    ProtocolError = 8,
}

impl DFUState {
    /// Create DFUState from a byte value
    pub fn from_byte(value: u8) -> Result<Self, ProtocolError> {
        match value {
            0 => Ok(DFUState::Idle),
            1 => Ok(DFUState::Preparing),
            2 => Ok(DFUState::Updating),
            3 => Ok(DFUState::QueueFull),
            4 => Ok(DFUState::ReadyToCommit),
            5 => Ok(DFUState::CrcError),
            6 => Ok(DFUState::DataLengthError),
            7 => Ok(DFUState::WriteError),
            8 => Ok(DFUState::ProtocolError),
            _ => Err(ProtocolError::InvalidDFUState(value)),
        }
    }

    /// Convert DFUState to byte value
    pub fn to_byte(self) -> u8 {
        self as u8
    }
}

// ============================================================================
// Encoding/Decoding Functions
// ============================================================================

/// Encode a 16-bit value as big-endian bytes
pub fn encode_word(value: u16) -> [u8; 2] {
    value.to_be_bytes()
}

/// Decode big-endian bytes to a 16-bit value
pub fn decode_word(bytes: &[u8]) -> Result<u16, ProtocolError> {
    if bytes.len() < 2 {
        return Err(ProtocolError::InsufficientData {
            expected: 2,
            got: bytes.len(),
        });
    }
    Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
}

/// Encode a 32-bit value as big-endian bytes
pub fn encode_u32(value: u32) -> [u8; 4] {
    value.to_be_bytes()
}

/// Decode big-endian bytes to a 32-bit value
pub fn decode_u32(bytes: &[u8]) -> Result<u32, ProtocolError> {
    if bytes.len() < 4 {
        return Err(ProtocolError::InsufficientData {
            expected: 4,
            got: bytes.len(),
        });
    }
    Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

// ============================================================================
// Analog Scaling Functions
// ============================================================================

/// Convert a 16-bit analog reading to a float value
///
/// # Arguments
/// * `raw` - Raw 16-bit value from I2C register
/// * `scale` - Full-scale value (e.g., 40.0 for 40V max)
///
/// # Returns
/// Scaled float value (e.g., voltage in volts)
pub fn analog_word_to_float(raw: u16, scale: f64) -> f64 {
    scale * (raw as f64) / 65536.0
}

/// Convert a float value to a 16-bit analog value
///
/// # Arguments
/// * `value` - Float value (e.g., voltage in volts)
/// * `scale` - Full-scale value (e.g., 40.0 for 40V max)
///
/// # Returns
/// 16-bit raw value for I2C register
pub fn float_to_analog_word(value: f64, scale: f64) -> u16 {
    ((65536.0 * value) / scale) as u16
}

/// Convert a byte analog reading to a float value (legacy, for firmware v2.x)
pub fn analog_byte_to_float(raw: u8, scale: f64) -> f64 {
    scale * (raw as f64) / 256.0
}

/// Convert a float value to a byte analog value (legacy, for firmware v2.x)
pub fn float_to_analog_byte(value: f64, scale: f64) -> u8 {
    ((256.0 * value) / scale) as u8
}

// ============================================================================
// Analog Scale Constants
// ============================================================================

/// Maximum supercapacitor voltage (11V)
pub const VCAP_MAX: f64 = 11.0;

/// Maximum DC input voltage (40V)
pub const DCIN_MAX: f64 = 40.0;

/// Maximum input current (3.3A)
pub const I_MAX: f64 = 3.3;

/// Minimum temperature in Kelvin (-40°C)
pub const TEMP_MIN_KELVIN: f64 = 273.15 - 40.0;

/// Maximum temperature in Kelvin (+100°C)
pub const TEMP_MAX_KELVIN: f64 = 273.15 + 100.0;

/// Temperature range (TEMP_MAX - TEMP_MIN)
pub const TEMP_RANGE_KELVIN: f64 = TEMP_MAX_KELVIN - TEMP_MIN_KELVIN;

/// Convert Kelvin to Celsius
pub fn kelvin_to_celsius(kelvin: f64) -> f64 {
    kelvin - 273.15
}

/// Convert Celsius to Kelvin
pub fn celsius_to_kelvin(celsius: f64) -> f64 {
    celsius + 273.15
}

// ============================================================================
// Errors
// ============================================================================

/// Protocol errors for I2C communication
#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("Invalid power state value: {0}")]
    InvalidPowerState(u8),

    #[error("Invalid DFU state value: {0}")]
    InvalidDFUState(u8),

    #[error("Insufficient data: expected {expected} bytes, got {got}")]
    InsufficientData { expected: usize, got: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_word() {
        let value: u16 = 0x1234;
        let bytes = encode_word(value);
        assert_eq!(bytes, [0x12, 0x34]);
        assert_eq!(decode_word(&bytes).unwrap(), value);
    }

    #[test]
    fn test_encode_decode_u32() {
        let value: u32 = 0x12345678;
        let bytes = encode_u32(value);
        assert_eq!(bytes, [0x12, 0x34, 0x56, 0x78]);
        assert_eq!(decode_u32(&bytes).unwrap(), value);
    }

    #[test]
    fn test_decode_insufficient_data() {
        assert!(decode_word(&[0x12]).is_err());
        assert!(decode_u32(&[0x12, 0x34]).is_err());
    }

    #[test]
    fn test_power_state_conversion() {
        assert_eq!(PowerState::from_byte(0).unwrap(), PowerState::PowerOff);
        assert_eq!(
            PowerState::from_byte(3).unwrap(),
            PowerState::OperationalSolo
        );
        assert_eq!(PowerState::from_byte(13).unwrap(), PowerState::Standby);
        assert!(PowerState::from_byte(14).is_err());

        assert_eq!(PowerState::PowerOff.to_byte(), 0);
        assert_eq!(PowerState::Standby.to_byte(), 13);
    }

    #[test]
    fn test_power_state_names() {
        assert_eq!(PowerState::PowerOff.name(), "PowerOff");
        assert_eq!(PowerState::OperationalSolo.name(), "OperationalSolo");
        assert_eq!(PowerState::Standby.name(), "Standby");
    }

    #[test]
    fn test_dfu_state_conversion() {
        assert_eq!(DFUState::from_byte(0).unwrap(), DFUState::Idle);
        assert_eq!(DFUState::from_byte(4).unwrap(), DFUState::ReadyToCommit);
        assert_eq!(DFUState::from_byte(8).unwrap(), DFUState::ProtocolError);
        assert!(DFUState::from_byte(9).is_err());

        assert_eq!(DFUState::Idle.to_byte(), 0);
        assert_eq!(DFUState::ProtocolError.to_byte(), 8);
    }

    #[test]
    fn test_analog_word_scaling() {
        // Test voltage scaling (40V max)
        let raw: u16 = 32768; // Half scale
        let voltage = analog_word_to_float(raw, DCIN_MAX);
        assert!((voltage - 20.0).abs() < 0.01); // Should be 20V

        // Round trip
        let raw_back = float_to_analog_word(voltage, DCIN_MAX);
        assert_eq!(raw_back, raw);
    }

    #[test]
    fn test_analog_byte_scaling() {
        // Test byte scaling (11V max)
        let raw: u8 = 128; // Half scale
        let voltage = analog_byte_to_float(raw, VCAP_MAX);
        assert!((voltage - 5.5).abs() < 0.1); // Should be ~5.5V

        // Round trip
        let raw_back = float_to_analog_byte(voltage, VCAP_MAX);
        assert_eq!(raw_back, raw);
    }

    #[test]
    fn test_temperature_conversion() {
        // 0°C should be 273.15K
        assert_eq!(celsius_to_kelvin(0.0), 273.15);
        assert_eq!(kelvin_to_celsius(273.15), 0.0);

        // 25°C should be 298.15K
        let kelvin = celsius_to_kelvin(25.0);
        assert!((kelvin - 298.15).abs() < 0.01);
        assert!((kelvin_to_celsius(kelvin) - 25.0).abs() < 0.01);
    }

    #[test]
    fn test_temperature_scaling() {
        // Temperature is stored as offset from TEMP_MIN
        // For 25°C (298.15K), offset from TEMP_MIN (233.15K) is 65K
        let temp_celsius = 25.0;
        let temp_kelvin = celsius_to_kelvin(temp_celsius);
        let offset = temp_kelvin - TEMP_MIN_KELVIN;

        // Encode as 16-bit value
        let raw = float_to_analog_word(offset, TEMP_RANGE_KELVIN);

        // Decode back
        let decoded_offset = analog_word_to_float(raw, TEMP_RANGE_KELVIN);
        let decoded_kelvin = decoded_offset + TEMP_MIN_KELVIN;
        let decoded_celsius = kelvin_to_celsius(decoded_kelvin);

        // Should match original (within tolerance)
        assert!((decoded_celsius - temp_celsius).abs() < 0.5);
    }
}
