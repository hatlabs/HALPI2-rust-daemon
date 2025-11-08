//! Shared types and utilities for HALPI2 daemon and CLI

pub mod config;
pub mod error;
pub mod protocol;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
