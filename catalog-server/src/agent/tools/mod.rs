//! Agent tool registry and execution.
//!
//! Tools are functions that agents can call to interact with external systems.
//! This module provides the trait definition and a registry for managing tools.

mod registry;

pub use registry::{AgentTool, AgentToolRegistry, ToolContext, ToolDefinition, ToolError};
