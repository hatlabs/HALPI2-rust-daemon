//! Shared types and utilities for HALPI2 daemon and CLI

pub mod config;
pub mod protocol;
pub mod types;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
