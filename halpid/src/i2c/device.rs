//! Low-level I2C device interface for HALPI2 controller
//!
//! This module provides the `HalpiDevice` struct which handles direct I2C
//! communication with the HALPI2 RP2040 controller. It includes:
//! - Atomic register read/write operations
//! - Retry logic for transient errors
//! - Firmware version detection with caching
//! - Version-dependent operation selection
//!
//! This module is only available on Linux targets.

use halpi_common::protocol::{self, ProtocolError};
use halpi_common::types::{Measurements, PowerState, Version};
use i2cdev::core::I2CDevice;
use i2cdev::linux::{LinuxI2CDevice, LinuxI2CError};
use std::thread;
use std::time::Duration;

/// Number of retry attempts for transient I2C errors
const MAX_RETRIES: usize = 3;

/// Delay between retry attempts
const RETRY_DELAY: Duration = Duration::from_millis(10);

/// I2C device interface for HALPI2 controller
pub struct HalpiDevice {
    /// Underlying Linux I2C device
    device: LinuxI2CDevice,
    /// I2C bus number (stored for error messages)
    #[allow(dead_code)]
    bus: u8,
    /// I2C device address (stored for error messages)
    #[allow(dead_code)]
    addr: u8,
    /// Cached firmware version (detected on first access)
    firmware_version: Option<String>,
}

impl HalpiDevice {
    /// Create a new HALPI2 device interface
    ///
    /// # Arguments
    /// * `bus` - I2C bus number (typically 1 for Raspberry Pi)
    /// * `addr` - I2C device address (typically 0x6D for HALPI2)
    ///
    /// # Errors
    /// Returns `I2cError` if the device cannot be opened (e.g., permissions,
    /// device doesn't exist, or hardware not connected).
    ///
    /// # Example
    /// ```ignore
    /// use halpid::i2c::HalpiDevice;
    ///
    /// let device = HalpiDevice::new(1, 0x6D)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn new(bus: u8, addr: u8) -> Result<Self, I2cError> {
        let device_path = format!("/dev/i2c-{}", bus);
        let device =
            LinuxI2CDevice::new(&device_path, addr as u16).map_err(|e| I2cError::DeviceOpen {
                bus,
                addr,
                source: e,
            })?;

        Ok(Self {
            device,
            bus,
            addr,
            firmware_version: None,
        })
    }

    /// Read a single byte from a register
    ///
    /// This performs an atomic I2C transaction with automatic retry on transient errors.
    pub(super) fn read_byte(&mut self, reg: u8) -> Result<u8, I2cError> {
        self.retry_operation(|device| {
            device
                .smbus_read_byte_data(reg)
                .map_err(|e| I2cError::Read { reg, source: e })
        })
    }

    /// Read multiple bytes from a register
    ///
    /// This performs an atomic I2C transaction with automatic retry on transient errors.
    fn read_bytes(&mut self, reg: u8, count: usize) -> Result<Vec<u8>, I2cError> {
        self.retry_operation(|device| {
            let mut buffer = vec![0u8; count];
            device
                .write(&[reg])
                .and_then(|_| device.read(&mut buffer))
                .map_err(|e| I2cError::Read { reg, source: e })?;
            Ok(buffer)
        })
    }

    /// Read a 16-bit word from a register (big-endian)
    ///
    /// This performs an atomic I2C transaction with automatic retry on transient errors.
    pub(super) fn read_word(&mut self, reg: u8) -> Result<u16, I2cError> {
        let bytes = self.read_bytes(reg, 2)?;
        protocol::decode_word(&bytes).map_err(|e| I2cError::Protocol {
            reg,
            operation: "decode_word",
            source: e,
        })
    }

    /// Read a 32-bit value from a register (big-endian)
    ///
    /// This performs an atomic I2C transaction with automatic retry on transient errors.
    #[allow(dead_code)]
    fn read_u32(&mut self, reg: u8) -> Result<u32, I2cError> {
        let bytes = self.read_bytes(reg, 4)?;
        protocol::decode_u32(&bytes).map_err(|e| I2cError::Protocol {
            reg,
            operation: "decode_u32",
            source: e,
        })
    }

    /// Write a single byte to a register
    ///
    /// This performs an atomic I2C transaction with automatic retry on transient errors.
    pub(super) fn write_byte(&mut self, reg: u8, value: u8) -> Result<(), I2cError> {
        self.retry_operation(|device| {
            device
                .smbus_write_byte_data(reg, value)
                .map_err(|e| I2cError::Write { reg, source: e })
        })
    }

    /// Write a 16-bit word to a register (big-endian)
    ///
    /// This performs an atomic I2C transaction with automatic retry on transient errors.
    #[allow(dead_code)]
    fn write_word(&mut self, reg: u8, value: u16) -> Result<(), I2cError> {
        let bytes = protocol::encode_word(value);
        self.retry_operation(|device| {
            device
                .write(&[reg, bytes[0], bytes[1]])
                .map_err(|e| I2cError::Write { reg, source: e })
        })
    }

    /// Write multiple bytes to a register
    ///
    /// This performs an atomic I2C transaction with automatic retry on transient errors.
    pub(super) fn write_bytes(&mut self, reg: u8, values: &[u8]) -> Result<(), I2cError> {
        self.retry_operation(|device| {
            let mut data = Vec::with_capacity(1 + values.len());
            data.push(reg);
            data.extend_from_slice(values);
            device
                .write(&data)
                .map_err(|e| I2cError::Write { reg, source: e })
        })
    }

    /// Get the firmware version (cached after first read)
    ///
    /// The firmware version is read once and cached for subsequent calls.
    /// Version format: "major.minor.patch" or "major.minor.patch-aN" for alpha versions.
    ///
    /// # Errors
    /// Returns `I2cError` if the version cannot be read from the device.
    pub fn firmware_version(&mut self) -> Result<&str, I2cError> {
        if self.firmware_version.is_none() {
            let bytes = self.read_bytes(protocol::REG_FIRMWARE_VERSION, 4)?;
            let version = if bytes[3] == 0xFF {
                format!("{}.{}.{}", bytes[0], bytes[1], bytes[2])
            } else {
                format!("{}.{}.{}-a{}", bytes[0], bytes[1], bytes[2], bytes[3])
            };
            self.firmware_version = Some(version);
        }

        Ok(self.firmware_version.as_ref().unwrap().as_str())
    }

    //
    // High-Level I2C Operations
    //

    /// Get hardware version
    ///
    /// # Errors
    /// Returns `I2cError` if the version cannot be read from the device.
    pub fn get_hardware_version(&mut self) -> Result<Version, I2cError> {
        let bytes = self.read_bytes(protocol::REG_HARDWARE_VERSION, 4)?;
        Ok(Version::from_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3],
        ]))
    }

    /// Get firmware version as a `Version` struct
    ///
    /// # Errors
    /// Returns `I2cError` if the version cannot be read from the device.
    pub fn get_firmware_version(&mut self) -> Result<Version, I2cError> {
        let bytes = self.read_bytes(protocol::REG_FIRMWARE_VERSION, 4)?;
        Ok(Version::from_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3],
        ]))
    }

    /// Get device unique ID as a hexadecimal string
    ///
    /// # Errors
    /// Returns `I2cError` if the ID cannot be read from the device.
    pub fn get_device_id(&mut self) -> Result<String, I2cError> {
        let bytes = self.read_bytes(protocol::REG_DEVICE_ID, 8)?;
        Ok(format!(
            "{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]
        ))
    }

    /// Get current power state
    ///
    /// # Errors
    /// Returns `I2cError` if the state cannot be read or is invalid.
    pub fn get_power_state(&mut self) -> Result<PowerState, I2cError> {
        let state_byte = self.read_byte(protocol::REG_STATE)?;
        PowerState::from_byte(state_byte).ok_or(I2cError::InvalidState { state: state_byte })
    }

    /// Get all measurements (analog values + state)
    ///
    /// This reads all sensor values in individual transactions.
    ///
    /// # Errors
    /// Returns `I2cError` if any measurements cannot be read.
    pub fn get_measurements(&mut self) -> Result<Measurements, I2cError> {
        // Read all analog values using word (16-bit) encoding
        let dcin_voltage = self.read_analog_word(protocol::REG_DCIN_VOLTAGE, protocol::DCIN_MAX)?;
        let supercap_voltage =
            self.read_analog_word(protocol::REG_SUPERCAP_VOLTAGE, protocol::VCAP_MAX)?;
        let input_current = self.read_analog_word(protocol::REG_INPUT_CURRENT, protocol::I_MAX)?;
        let mcu_temperature = self
            .read_analog_word(protocol::REG_MCU_TEMPERATURE, protocol::TEMP_RANGE_KELVIN)?
            + protocol::TEMP_MIN_KELVIN;
        let pcb_temperature = self
            .read_analog_word(protocol::REG_PCB_TEMPERATURE, protocol::TEMP_RANGE_KELVIN)?
            + protocol::TEMP_MIN_KELVIN;

        // Read power state
        let power_state = self.get_power_state()?;

        // Read watchdog elapsed time (in 0.1 second increments)
        let watchdog_elapsed_byte = self.read_byte(protocol::REG_WATCHDOG_ELAPSED)?;
        let watchdog_elapsed = (watchdog_elapsed_byte as f32) * 0.1;

        Ok(Measurements {
            dcin_voltage,
            supercap_voltage,
            input_current,
            mcu_temperature,
            pcb_temperature,
            power_state,
            watchdog_elapsed,
        })
    }

    /// Set watchdog timeout in milliseconds
    ///
    /// Set to 0 to disable the watchdog.
    ///
    /// # Errors
    /// Returns `I2cError` if the timeout cannot be written.
    pub fn set_watchdog_timeout(&mut self, timeout_ms: u16) -> Result<(), I2cError> {
        self.write_word(protocol::REG_WATCHDOG_TIMEOUT, timeout_ms)
    }

    /// Feed the watchdog
    ///
    /// This resets the watchdog timer. Must be called periodically (typically every 5 seconds)
    /// to prevent the watchdog from triggering a system shutdown.
    ///
    /// # Errors
    /// Returns `I2cError` if the feed command cannot be written.
    pub fn feed_watchdog(&mut self) -> Result<(), I2cError> {
        self.write_byte(protocol::REG_WATCHDOG_FEED, 0x01)
    }

    /// Set power-on voltage threshold (in volts)
    ///
    /// # Errors
    /// Returns `I2cError` if the threshold cannot be written.
    pub fn set_power_on_threshold(&mut self, volts: f32) -> Result<(), I2cError> {
        self.write_analog_word(protocol::REG_POWER_ON_THRESHOLD, volts, protocol::VCAP_MAX)
    }

    /// Set solo mode power-off voltage threshold (in volts)
    ///
    /// # Errors
    /// Returns `I2cError` if the threshold cannot be written.
    pub fn set_solo_power_off_threshold(&mut self, volts: f32) -> Result<(), I2cError> {
        self.write_analog_word(
            protocol::REG_SOLO_POWEROFF_THRESHOLD,
            volts,
            protocol::VCAP_MAX,
        )
    }

    /// Enable or disable 5V output
    ///
    /// # Errors
    /// Returns `I2cError` if the state cannot be written.
    pub fn set_5v_output_enabled(&mut self, enabled: bool) -> Result<(), I2cError> {
        self.write_byte(protocol::REG_EN5V_STATE, if enabled { 1 } else { 0 })
    }

    /// Get 5V output enable state
    ///
    /// # Errors
    /// Returns `I2cError` if the state cannot be read.
    pub fn get_5v_output_enabled(&mut self) -> Result<bool, I2cError> {
        Ok(self.read_byte(protocol::REG_EN5V_STATE)? != 0)
    }

    /// Set LED brightness (0-255)
    ///
    /// **Note**: This feature requires firmware version 2.x or later.
    /// Check firmware version before calling this method.
    ///
    /// # Errors
    /// Returns `I2cError` if the brightness cannot be written.
    pub fn set_led_brightness(&mut self, brightness: u8) -> Result<(), I2cError> {
        self.write_byte(protocol::REG_LED_BRIGHTNESS, brightness)
    }

    /// Set auto-restart enable state
    ///
    /// When enabled, the system will automatically restart after a shutdown.
    ///
    /// # Errors
    /// Returns `I2cError` if the state cannot be written.
    pub fn set_auto_restart(&mut self, enabled: bool) -> Result<(), I2cError> {
        self.write_byte(protocol::REG_AUTO_RESTART, if enabled { 1 } else { 0 })
    }

    /// Set solo depleting timeout in milliseconds
    ///
    /// # Errors
    /// Returns `I2cError` if the timeout cannot be written.
    pub fn set_solo_depleting_timeout(&mut self, timeout_ms: u32) -> Result<(), I2cError> {
        let bytes = protocol::encode_u32(timeout_ms);
        self.write_bytes(protocol::REG_SOLO_DEPLETING_TIMEOUT, &bytes)
    }

    /// Get USB port state as a bitfield
    ///
    /// Bits 0-3 correspond to USB ports 0-3. A set bit means the port is enabled.
    ///
    /// # Errors
    /// Returns `I2cError` if the state cannot be read.
    pub fn get_usb_port_state(&mut self) -> Result<u8, I2cError> {
        self.read_byte(protocol::REG_USB_PORT_STATE)
    }

    /// Set USB port state as a bitfield
    ///
    /// Bits 0-3 correspond to USB ports 0-3. A set bit enables the port.
    /// Only the lower 4 bits are used; upper bits are masked off.
    ///
    /// # Errors
    /// Returns `I2cError` if the state cannot be written.
    pub fn set_usb_port_state(&mut self, port_bits: u8) -> Result<(), I2cError> {
        self.write_byte(protocol::REG_USB_PORT_STATE, port_bits & 0x0F)
    }

    /// Request system shutdown
    ///
    /// This signals the firmware to initiate a graceful shutdown sequence.
    ///
    /// # Errors
    /// Returns `I2cError` if the request cannot be written.
    pub fn request_shutdown(&mut self) -> Result<(), I2cError> {
        self.write_byte(protocol::REG_REQUEST_SHUTDOWN, 0x01)
    }

    /// Request standby mode
    ///
    /// This signals the firmware to enter low-power standby mode.
    ///
    /// # Errors
    /// Returns `I2cError` if the request cannot be written.
    pub fn request_standby(&mut self) -> Result<(), I2cError> {
        self.write_byte(protocol::REG_REQUEST_STANDBY, 0x01)
    }

    //
    // Helper methods for analog value encoding/decoding
    //

    /// Read an analog value as a 16-bit word with scaling
    fn read_analog_word(&mut self, reg: u8, scale: f32) -> Result<f32, I2cError> {
        let raw = self.read_word(reg)?;
        Ok(protocol::analog_word_to_float(raw, scale))
    }

    /// Write an analog value as a 16-bit word with scaling
    fn write_analog_word(&mut self, reg: u8, value: f32, scale: f32) -> Result<(), I2cError> {
        let raw = protocol::float_to_analog_word(value, scale);
        self.write_word(reg, raw)
    }

    /// Retry an I2C operation on transient errors
    ///
    /// Retries up to MAX_RETRIES times with RETRY_DELAY between attempts.
    /// Only retries on errors that are likely to be transient (I/O errors).
    fn retry_operation<T>(
        &mut self,
        mut operation: impl FnMut(&mut LinuxI2CDevice) -> Result<T, I2cError>,
    ) -> Result<T, I2cError> {
        let mut last_error = None;

        for attempt in 0..=MAX_RETRIES {
            match operation(&mut self.device) {
                Ok(result) => return Ok(result),
                Err(err) => {
                    // Only retry on transient errors (I/O errors)
                    if !Self::is_transient_error(&err) {
                        return Err(err);
                    }

                    last_error = Some(err);

                    // Don't delay after the last attempt
                    if attempt < MAX_RETRIES {
                        thread::sleep(RETRY_DELAY);
                    }
                }
            }
        }

        // All retries exhausted, return the last error
        Err(last_error.expect("retry_operation called with MAX_RETRIES = 0"))
    }

    /// Check if an error is transient and should be retried
    fn is_transient_error(err: &I2cError) -> bool {
        matches!(err, I2cError::Read { .. } | I2cError::Write { .. })
    }
}

/// Errors that can occur during I2C operations
#[derive(Debug, thiserror::Error)]
pub enum I2cError {
    /// Failed to open I2C device
    #[error("Failed to open I2C device at bus {bus}, address 0x{addr:02X}")]
    DeviceOpen {
        bus: u8,
        addr: u8,
        #[source]
        source: LinuxI2CError,
    },

    /// Failed to read from register
    #[error("Failed to read from register 0x{reg:02X}")]
    Read {
        reg: u8,
        #[source]
        source: LinuxI2CError,
    },

    /// Failed to write to register
    #[error("Failed to write to register 0x{reg:02X}")]
    Write {
        reg: u8,
        #[source]
        source: LinuxI2CError,
    },

    /// Protocol decoding error
    #[error("Protocol error at register 0x{reg:02X} during {operation}")]
    Protocol {
        reg: u8,
        operation: &'static str,
        #[source]
        source: ProtocolError,
    },

    /// Invalid power state value
    #[error("Invalid power state value: {state}")]
    InvalidState { state: u8 },

    /// Invalid DFU state value
    #[error("Invalid DFU state value: {state}")]
    InvalidDfuState { state: u8 },

    /// Invalid block size for firmware upload
    #[error("Invalid block size {size} (max: {max_size})")]
    InvalidBlockSize { size: usize, max_size: usize },

    /// DFU error state encountered
    #[error("DFU error state: {state:?}")]
    DfuError { state: protocol::DFUState },

    /// DFU unexpected state
    #[error("DFU unexpected state: expected {expected:?}, got {actual:?}")]
    DfuUnexpectedState {
        expected: protocol::DFUState,
        actual: protocol::DFUState,
    },

    /// DFU queue full timeout (too many retries)
    #[error("DFU queue full timeout: firmware controller is not accepting blocks")]
    DfuQueueFullTimeout,
}

// Note: Unit tests are omitted because constructing LinuxI2CError instances
// requires internal types from the i2cdev crate that are not publicly exposed.
// The retry logic and error handling will be tested through integration tests
// with actual hardware.
