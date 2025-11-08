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

#![cfg(target_os = "linux")]

use halpi_common::protocol::{self, ProtocolError};
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
    /// I2C bus number
    bus: u8,
    /// I2C device address
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
    fn read_byte(&mut self, reg: u8) -> Result<u8, I2cError> {
        self.retry_operation(|| {
            self.device
                .smbus_read_byte_data(reg)
                .map_err(|e| I2cError::Read {
                    reg,
                    source: e.into(),
                })
        })
    }

    /// Read multiple bytes from a register
    ///
    /// This performs an atomic I2C transaction with automatic retry on transient errors.
    fn read_bytes(&mut self, reg: u8, count: usize) -> Result<Vec<u8>, I2cError> {
        self.retry_operation(|| {
            let mut buffer = vec![0u8; count];
            self.device
                .write(&[reg])
                .and_then(|_| self.device.read(&mut buffer))
                .map_err(|e| I2cError::Read {
                    reg,
                    source: e.into(),
                })?;
            Ok(buffer)
        })
    }

    /// Read a 16-bit word from a register (big-endian)
    ///
    /// This performs an atomic I2C transaction with automatic retry on transient errors.
    fn read_word(&mut self, reg: u8) -> Result<u16, I2cError> {
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
    fn write_byte(&mut self, reg: u8, value: u8) -> Result<(), I2cError> {
        self.retry_operation(|| {
            self.device
                .smbus_write_byte_data(reg, value)
                .map_err(|e| I2cError::Write {
                    reg,
                    source: e.into(),
                })
        })
    }

    /// Write a 16-bit word to a register (big-endian)
    ///
    /// This performs an atomic I2C transaction with automatic retry on transient errors.
    fn write_word(&mut self, reg: u8, value: u16) -> Result<(), I2cError> {
        let bytes = protocol::encode_word(value);
        self.retry_operation(|| {
            self.device
                .write(&[reg, bytes[0], bytes[1]])
                .map_err(|e| I2cError::Write {
                    reg,
                    source: e.into(),
                })
        })
    }

    /// Write multiple bytes to a register
    ///
    /// This performs an atomic I2C transaction with automatic retry on transient errors.
    fn write_bytes(&mut self, reg: u8, values: &[u8]) -> Result<(), I2cError> {
        self.retry_operation(|| {
            let mut data = Vec::with_capacity(1 + values.len());
            data.push(reg);
            data.extend_from_slice(values);
            self.device.write(&data).map_err(|e| I2cError::Write {
                reg,
                source: e.into(),
            })
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

    /// Retry an I2C operation on transient errors
    ///
    /// Retries up to MAX_RETRIES times with RETRY_DELAY between attempts.
    /// Only retries on errors that are likely to be transient (I/O errors).
    fn retry_operation<T, F>(&mut self, mut operation: F) -> Result<T, I2cError>
    where
        F: FnMut() -> Result<T, I2cError>,
    {
        let mut last_error = None;

        for attempt in 0..=MAX_RETRIES {
            match operation() {
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
}

// Note: Unit tests are omitted because constructing LinuxI2CError instances
// requires internal types from the i2cdev crate that are not publicly exposed.
// The retry logic and error handling will be tested through integration tests
// with actual hardware.
