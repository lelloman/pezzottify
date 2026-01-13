//! LLM provider trait definition.

use super::types::{CompletionResponse, Message};
use crate::agent::tools::ToolDefinition;
use async_trait::async_trait;
use std::time::Duration;
use thiserror::Error;

/// Options for a completion request.
#[derive(Debug, Clone)]
pub struct CompletionOptions {
    /// Temperature for sampling (0.0 = deterministic, 1.0 = creative).
    pub temperature: f32,
    /// Maximum tokens to generate.
    pub max_tokens: Option<u32>,
    /// Request timeout.
    pub timeout: Duration,
}

impl Default for CompletionOptions {
    fn default() -> Self {
        Self {
            temperature: 0.3,
            max_tokens: None,
            timeout: Duration::from_secs(120),
        }
    }
}

/// Errors that can occur when interacting with an LLM provider.
#[derive(Debug, Error)]
pub enum LlmError {
    #[error("Connection error: {0}")]
    Connection(String),

    #[error("API error (status {status}): {message}")]
    Api { status: u16, message: String },

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("Rate limited")]
    RateLimited,

    #[error("Request timeout")]
    Timeout,

    #[error("Serialization error: {0}")]
    Serialization(String),
}

/// Trait for LLM providers.
///
/// Implementations of this trait can connect to different LLM backends
/// (Ollama, OpenAI, Anthropic, etc.) while providing a unified interface.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Get the provider's name (e.g., "ollama", "openai").
    fn name(&self) -> &str;

    /// Get the model being used.
    fn model(&self) -> &str;

    /// Complete a conversation, optionally with tool support.
    ///
    /// # Arguments
    /// * `messages` - The conversation history.
    /// * `tools` - Optional tool definitions the model can use.
    /// * `options` - Completion options (temperature, timeout, etc.).
    ///
    /// # Returns
    /// The model's response, which may include tool calls.
    async fn complete(
        &self,
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
        options: &CompletionOptions,
    ) -> Result<CompletionResponse, LlmError>;

    /// Check if the provider is healthy and reachable.
    async fn health_check(&self) -> Result<(), LlmError>;
}
