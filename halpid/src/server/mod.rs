//! HTTP server for the HALPI2 daemon
//!
//! This module implements the Axum-based HTTP server that exposes the
//! daemon's API over a Unix domain socket.

pub mod app;
pub mod handlers;

pub use app::{AppState, create_app};
