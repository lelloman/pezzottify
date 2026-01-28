//! WebSocket infrastructure for real-time communication.
//!
//! This module provides generic WebSocket support that can be extended
//! for features like user data sync, remote playback control, and notifications.

#![allow(unused_imports)]

pub mod connection;
pub mod handler;
pub mod messages;
pub mod playback_messages;

pub use connection::ConnectionManager;
pub use handler::ws_handler;
pub use messages::{ClientMessage, ServerMessage};
