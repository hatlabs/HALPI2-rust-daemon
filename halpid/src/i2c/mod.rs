//! I2C device communication for HALPI2 hardware
//!
//! This module is only available on Linux targets where I2C device drivers are present.

pub mod device;

pub mod dfu;

pub use device::HalpiDevice;
