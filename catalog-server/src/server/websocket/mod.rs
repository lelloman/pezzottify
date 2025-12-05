//! WebSocket infrastructure for real-time communication.
//!
//! This module provides generic WebSocket support that can be extended
//! for features like user data sync, remote playback control, and notifications.

pub mod connection;
pub mod messages;
// pub mod handler;

pub use connection::ConnectionManager;
pub use messages::{ClientMessage, ServerMessage};
