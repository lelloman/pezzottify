//! LLM provider abstraction layer.
//!
//! This module provides a trait-based abstraction for LLM providers,
//! allowing the agent to work with different backends (Ollama, OpenAI, etc.).

mod ollama;
mod openai;
mod provider;
mod types;

pub use ollama::OllamaProvider;
pub use openai::OpenAIProvider;
pub use provider::{CompletionOptions, LlmError, LlmProvider};
pub use types::{CompletionResponse, FinishReason, Message, MessageRole, ToolCall};
