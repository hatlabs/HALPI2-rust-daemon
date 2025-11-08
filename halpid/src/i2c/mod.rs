//! I2C device communication for HALPI2 hardware
//!
//! This module is only available on Linux targets where I2C device drivers are present.

#[cfg(target_os = "linux")]
pub mod device;

#[cfg(target_os = "linux")]
pub use device::HalpiDevice;
