//! Agent infrastructure for LLM-powered workflows.
//!
//! This module provides a generic agent framework that can be used to build
//! LLM-powered features. It includes:
//! - LLM provider abstraction (supports Ollama, extensible to others)
//! - Tool registry for agent capabilities
//! - Workflow state machine for step-by-step execution
//! - Reasoning logger for observability

pub mod llm;
pub mod reasoning;
pub mod tools;
pub mod workflow;

pub use llm::{
    CompletionOptions, CompletionResponse, LlmError, LlmProvider, Message, MessageRole,
    OllamaProvider,
};
pub use reasoning::{ReasoningLogger, ReasoningStep, ReasoningStepType};
pub use tools::{AgentTool, AgentToolRegistry, ToolContext, ToolDefinition, ToolError};
pub use workflow::{AgentAction, Workflow, WorkflowError, WorkflowExecutor, WorkflowState};
