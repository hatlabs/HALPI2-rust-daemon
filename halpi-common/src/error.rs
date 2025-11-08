//! Error types for HALPI2 daemon and CLI
//!
//! This module provides a comprehensive error type hierarchy for the HALPI2 system:
//! - AppError: Top-level application errors
//! - I2cError: I2C communication errors
//! - ServerError: HTTP server errors
//! - ConfigError: Configuration loading/validation errors (re-exported from config module)
//! - ProtocolError: I2C protocol errors (re-exported from protocol module)

use std::io;
use std::path::PathBuf;

pub use crate::config::ConfigError;
pub use crate::protocol::ProtocolError;

// ============================================================================
// Top-Level Application Error
// ============================================================================

/// Top-level application error type
///
/// This is the main error type used throughout the application. It wraps
/// all lower-level errors and provides context for debugging.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// I2C communication error
    #[error("I2C communication error: {0}")]
    I2c(#[from] I2cError),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    /// Protocol error
    #[error("Protocol error: {0}")]
    Protocol(#[from] ProtocolError),

    /// Server error
    #[error("Server error: {0}")]
    Server(#[from] ServerError),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// Watchdog timeout
    #[error("Watchdog timeout: device did not respond within {0} seconds")]
    WatchdogTimeout(f64),

    /// Invalid state transition
    #[error("Invalid state transition: cannot transition from {from} to {to}")]
    InvalidStateTransition { from: String, to: String },

    /// Shutdown failed
    #[error("Shutdown failed: {0}")]
    ShutdownFailed(String),

    /// Generic error with context
    #[error("{0}")]
    Other(String),
}

// ============================================================================
// I2C Error
// ============================================================================

/// I2C-specific communication errors
#[derive(Debug, thiserror::Error)]
pub enum I2cError {
    /// Failed to open I2C device
    #[error("Failed to open I2C device {device}: {source}")]
    DeviceOpen {
        device: String,
        #[source]
        source: io::Error,
    },

    /// Failed to set I2C slave address
    #[error("Failed to set I2C slave address 0x{address:02X}: {source}")]
    SetSlaveAddress {
        address: u8,
        #[source]
        source: io::Error,
    },

    /// Read operation failed
    #[error("Failed to read from register 0x{register:02X}: {source}")]
    ReadFailed {
        register: u8,
        #[source]
        source: io::Error,
    },

    /// Write operation failed
    #[error("Failed to write to register 0x{register:02X}: {source}")]
    WriteFailed {
        register: u8,
        #[source]
        source: io::Error,
    },

    /// Invalid register address
    #[error("Invalid register address: 0x{0:02X}")]
    InvalidRegister(u8),

    /// Unexpected read length
    #[error("Unexpected read length: expected {expected} bytes, got {got}")]
    UnexpectedReadLength { expected: usize, got: usize },

    /// Device not responding
    #[error("I2C device at address 0x{0:02X} is not responding")]
    DeviceNotResponding(u8),

    /// Bus error (e.g., arbitration lost, clock stretching timeout)
    #[error("I2C bus error: {0}")]
    BusError(String),
}

// ============================================================================
// Server Error
// ============================================================================

/// HTTP server errors
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    /// Failed to bind to socket
    #[error("Failed to bind to socket {path:?}: {source}")]
    BindFailed {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    /// Failed to set socket permissions
    #[error("Failed to set permissions on socket {path:?}: {source}")]
    SetPermissionsFailed {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    /// Failed to change socket group ownership
    #[error("Failed to change socket group to {group}: {source}")]
    ChangeGroupFailed {
        group: String,
        #[source]
        source: io::Error,
    },

    /// Invalid request
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Missing required field
    #[error("Missing required field: {0}")]
    MissingField(String),

    /// Invalid parameter value
    #[error("Invalid parameter value for {field}: {reason}")]
    InvalidParameter { field: String, reason: String },

    /// Resource not found
    #[error("Resource not found: {0}")]
    NotFound(String),

    /// Method not allowed
    #[error("Method not allowed: {0}")]
    MethodNotAllowed(String),

    /// Internal server error
    #[error("Internal server error: {0}")]
    Internal(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Server startup failed
    #[error("Server startup failed: {0}")]
    StartupFailed(String),

    /// Server shutdown failed
    #[error("Server shutdown failed: {0}")]
    ShutdownFailed(String),
}

// ============================================================================
// Result Type Aliases
// ============================================================================

/// Result type using AppError
pub type Result<T> = std::result::Result<T, AppError>;

/// Result type using I2cError
pub type I2cResult<T> = std::result::Result<T, I2cError>;

/// Result type using ServerError
pub type ServerResult<T> = std::result::Result<T, ServerError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_error_from_i2c_error() {
        let i2c_err = I2cError::DeviceNotResponding(0x6D);
        let app_err: AppError = i2c_err.into();
        assert!(matches!(app_err, AppError::I2c(_)));
        assert!(app_err.to_string().contains("0x6D"));
    }

    #[test]
    fn test_app_error_from_config_error() {
        let config_err = ConfigError::InvalidValue("test".to_string());
        let app_err: AppError = config_err.into();
        assert!(matches!(app_err, AppError::Config(_)));
    }

    #[test]
    fn test_i2c_error_device_open() {
        let err = I2cError::DeviceOpen {
            device: "/dev/i2c-1".to_string(),
            source: io::Error::new(io::ErrorKind::NotFound, "device not found"),
        };
        let msg = err.to_string();
        assert!(msg.contains("/dev/i2c-1"));
        assert!(msg.contains("device not found"));
    }

    #[test]
    fn test_i2c_error_read_failed() {
        let err = I2cError::ReadFailed {
            register: 0x20,
            source: io::Error::new(io::ErrorKind::TimedOut, "timeout"),
        };
        let msg = err.to_string();
        assert!(msg.contains("0x20"));
        assert!(msg.contains("timeout"));
    }

    #[test]
    fn test_i2c_error_write_failed() {
        let err = I2cError::WriteFailed {
            register: 0x10,
            source: io::Error::new(io::ErrorKind::BrokenPipe, "pipe broken"),
        };
        let msg = err.to_string();
        assert!(msg.contains("0x10"));
        assert!(msg.contains("pipe broken"));
    }

    #[test]
    fn test_i2c_error_unexpected_read_length() {
        let err = I2cError::UnexpectedReadLength {
            expected: 4,
            got: 2,
        };
        let msg = err.to_string();
        assert!(msg.contains("expected 4"));
        assert!(msg.contains("got 2"));
    }

    #[test]
    fn test_server_error_bind_failed() {
        let err = ServerError::BindFailed {
            path: PathBuf::from("/run/halpid.sock"),
            source: io::Error::new(io::ErrorKind::PermissionDenied, "permission denied"),
        };
        let msg = err.to_string();
        assert!(msg.contains("/run/halpid.sock"));
        assert!(msg.contains("permission denied"));
    }

    #[test]
    fn test_server_error_invalid_request() {
        let err = ServerError::InvalidRequest("malformed JSON".to_string());
        let msg = err.to_string();
        assert!(msg.contains("Invalid request"));
        assert!(msg.contains("malformed JSON"));
    }

    #[test]
    fn test_server_error_invalid_parameter() {
        let err = ServerError::InvalidParameter {
            field: "voltage".to_string(),
            reason: "must be between 5.0 and 15.0".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("voltage"));
        assert!(msg.contains("must be between 5.0 and 15.0"));
    }

    #[test]
    fn test_app_error_watchdog_timeout() {
        let err = AppError::WatchdogTimeout(30.0);
        let msg = err.to_string();
        assert!(msg.contains("Watchdog timeout"));
        assert!(msg.contains("30"));
    }

    #[test]
    fn test_app_error_invalid_state_transition() {
        let err = AppError::InvalidStateTransition {
            from: "PowerOff".to_string(),
            to: "Standby".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("PowerOff"));
        assert!(msg.contains("Standby"));
    }

    #[test]
    fn test_error_chain_propagation() {
        // Test that errors can be chained properly
        fn inner() -> I2cResult<()> {
            Err(I2cError::DeviceNotResponding(0x6D))
        }

        fn outer() -> Result<()> {
            inner()?;
            Ok(())
        }

        let result = outer();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, AppError::I2c(_)));
    }

    #[test]
    fn test_error_display_messages_are_clear() {
        // Ensure error messages are user-friendly
        let errors = vec![
            AppError::WatchdogTimeout(10.0),
            AppError::ShutdownFailed("system busy".to_string()),
            AppError::I2c(I2cError::DeviceNotResponding(0x6D)),
        ];

        for err in errors {
            let msg = err.to_string();
            // Messages should not be empty and should be descriptive
            assert!(!msg.is_empty());
            assert!(msg.len() > 10); // Reasonably descriptive
        }
    }
}
