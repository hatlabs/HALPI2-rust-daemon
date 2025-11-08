//! Power management state machine

pub mod machine;

pub use machine::DaemonState;

#[cfg(target_os = "linux")]
pub use machine::StateMachine;
