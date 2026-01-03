//! MCP (Model Context Protocol) Server
//!
//! Provides an MCP interface for LLM-based administration, debugging,
//! and catalog management. Exposes tools and resources that LLM clients
//! can use to interact with the catalog server.
//!
//! ## Architecture
//!
//! - Transport: WebSocket at `/v1/mcp`
//! - Auth: Same as HTTP API (session-based)
//! - Tools: Permission-gated, consolidated for context efficiency
//! - Resources: Read-only data access (logs, job output, config)

pub mod context;
pub mod handler;
pub mod protocol;
pub mod rate_limit;
pub mod registry;
pub mod resources;
pub mod tools;

pub use handler::mcp_handler;
pub use protocol::{McpError, McpRequest, McpResponse};
pub use registry::McpRegistry;
