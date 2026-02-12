//! OpenAI-compatible LLM provider implementation.
//!
//! Works with OpenAI, OpenRouter, Together AI, vLLM, and any other
//! service implementing the OpenAI chat completions API.

use super::provider::{CompletionOptions, LlmError, LlmProvider};
use super::types::{CompletionResponse, FinishReason, Message, MessageRole, TokenUsage, ToolCall};
use crate::agent::tools::ToolDefinition;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::process::Command;
use tracing::{debug, warn};

/// Timeout for api_key_command execution.
const API_KEY_COMMAND_TIMEOUT: Duration = Duration::from_secs(10);

/// Source of API key for authentication.
#[derive(Debug, Clone)]
pub enum ApiKeySource {
    /// No authentication.
    None,
    /// Static API key.
    Static(String),
    /// Shell command that outputs the API key (for rotating tokens).
    Command(String),
}

impl ApiKeySource {
    /// Get the current API key, executing the command if necessary.
    async fn get_key(&self) -> Result<Option<String>, LlmError> {
        match self {
            ApiKeySource::None => Ok(None),
            ApiKeySource::Static(key) => Ok(Some(key.clone())),
            ApiKeySource::Command(cmd) => {
                debug!(command = %cmd, "Fetching API key via command");

                let result = tokio::time::timeout(
                    API_KEY_COMMAND_TIMEOUT,
                    Command::new("sh").arg("-c").arg(cmd).output(),
                )
                .await;

                let output = match result {
                    Ok(Ok(output)) => output,
                    Ok(Err(e)) => {
                        warn!(command = %cmd, error = %e, "api_key_command failed to execute");
                        return Err(LlmError::Connection(format!(
                            "Failed to execute api_key_command: {}",
                            e
                        )));
                    }
                    Err(_) => {
                        warn!(command = %cmd, "api_key_command timed out");
                        return Err(LlmError::Timeout);
                    }
                };

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    warn!(command = %cmd, stderr = %stderr, "api_key_command failed");
                    return Err(LlmError::Connection(format!(
                        "api_key_command failed with status {}: {}",
                        output.status, stderr
                    )));
                }

                let key = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if key.is_empty() {
                    warn!(command = %cmd, "api_key_command returned empty key");
                    return Err(LlmError::Connection(
                        "api_key_command returned empty key".to_string(),
                    ));
                }

                Ok(Some(key))
            }
        }
    }
}

/// OpenAI-compatible LLM provider.
///
/// Connects to any service implementing the OpenAI chat completions API.
pub struct OpenAIProvider {
    client: Client,
    base_url: String,
    model: String,
    api_key_source: ApiKeySource,
}

impl OpenAIProvider {
    /// Create a new OpenAI-compatible provider with a static API key.
    ///
    /// # Arguments
    /// * `base_url` - Base URL of the API (e.g., "https://api.openai.com/v1").
    /// * `model` - Model to use (e.g., "gpt-4o", "gpt-4o-mini").
    /// * `api_key` - Optional static API key for authentication.
    pub fn new(
        base_url: impl Into<String>,
        model: impl Into<String>,
        api_key: Option<String>,
    ) -> Self {
        let api_key_source = match api_key {
            Some(key) => ApiKeySource::Static(key),
            None => ApiKeySource::None,
        };
        Self {
            client: Client::new(),
            base_url: base_url.into(),
            model: model.into(),
            api_key_source,
        }
    }

    /// Create a new OpenAI-compatible provider with a command-based API key.
    ///
    /// The command is executed before each request to get a fresh token.
    /// This is useful for rotating tokens or fetching from secret stores.
    ///
    /// # Arguments
    /// * `base_url` - Base URL of the API.
    /// * `model` - Model to use.
    /// * `api_key_command` - Shell command that outputs the API key.
    pub fn with_key_command(
        base_url: impl Into<String>,
        model: impl Into<String>,
        api_key_command: String,
    ) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into(),
            model: model.into(),
            api_key_source: ApiKeySource::Command(api_key_command),
        }
    }

    /// Convert our messages to OpenAI's format.
    fn to_openai_messages(messages: &[Message]) -> Vec<OpenAIMessage> {
        messages.iter().map(|m| m.into()).collect()
    }

    /// Convert tool definitions to OpenAI's format.
    fn to_openai_tools(tools: &[ToolDefinition]) -> Vec<OpenAITool> {
        tools.iter().map(|t| t.into()).collect()
    }
}

#[async_trait]
impl LlmProvider for OpenAIProvider {
    fn name(&self) -> &str {
        "openai"
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn complete(
        &self,
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
        options: &CompletionOptions,
    ) -> Result<CompletionResponse, LlmError> {
        let url = format!("{}/chat/completions", self.base_url);

        let request = OpenAIChatRequest {
            model: self.model.clone(),
            messages: Self::to_openai_messages(messages),
            tools: tools.map(Self::to_openai_tools),
            temperature: Some(options.temperature),
            max_tokens: options.max_tokens,
        };

        debug!(
            model = %self.model,
            message_count = messages.len(),
            has_tools = tools.is_some(),
            "Sending completion request to OpenAI-compatible API"
        );

        let mut req_builder = self.client.post(&url).json(&request);

        if let Some(api_key) = self.api_key_source.get_key().await? {
            req_builder = req_builder.header("Authorization", format!("Bearer {}", api_key));
        }

        let response = req_builder
            .timeout(options.timeout)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    LlmError::Timeout
                } else {
                    LlmError::Connection(e.to_string())
                }
            })?;

        let status = response.status();
        if status.as_u16() == 429 {
            return Err(LlmError::RateLimited);
        }
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(LlmError::Api {
                status: status.as_u16(),
                message: body,
            });
        }

        let openai_response: OpenAIChatResponse = response.json().await.map_err(|e| {
            LlmError::InvalidResponse(format!("Failed to parse OpenAI response: {}", e))
        })?;

        // Get the first choice (there should always be at least one)
        let choice = openai_response.choices.into_iter().next().ok_or_else(|| {
            LlmError::InvalidResponse("No choices in OpenAI response".to_string())
        })?;

        let has_tool_calls = choice
            .message
            .tool_calls
            .as_ref()
            .map(|tc| !tc.is_empty())
            .unwrap_or(false);

        let tool_calls = choice.message.tool_calls.map(|calls| {
            calls
                .into_iter()
                .map(|tc| ToolCall {
                    id: tc.id,
                    name: tc.function.name,
                    arguments: serde_json::from_str(&tc.function.arguments)
                        .unwrap_or(serde_json::Value::Object(serde_json::Map::new())),
                })
                .collect()
        });

        let message = Message {
            role: MessageRole::Assistant,
            content: choice.message.content.unwrap_or_default(),
            tool_calls,
            tool_call_id: None,
            tool_name: None,
        };

        let finish_reason = match choice.finish_reason.as_deref() {
            Some("tool_calls") => FinishReason::ToolCalls,
            Some("length") => FinishReason::MaxTokens,
            _ if has_tool_calls => FinishReason::ToolCalls,
            _ => FinishReason::Stop,
        };

        let usage = openai_response.usage.map(|u| TokenUsage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        });

        debug!(
            finish_reason = ?finish_reason,
            has_tool_calls = has_tool_calls,
            "Received completion response from OpenAI-compatible API"
        );

        Ok(CompletionResponse {
            message,
            finish_reason,
            usage,
        })
    }

    async fn health_check(&self) -> Result<(), LlmError> {
        let url = format!("{}/models", self.base_url);

        let mut req_builder = self
            .client
            .get(&url)
            .timeout(std::time::Duration::from_secs(5));

        if let Some(api_key) = self.api_key_source.get_key().await? {
            req_builder = req_builder.header("Authorization", format!("Bearer {}", api_key));
        }

        let response = req_builder.send().await.map_err(|e| {
            if e.is_timeout() {
                LlmError::Timeout
            } else {
                LlmError::Connection(e.to_string())
            }
        })?;

        if !response.status().is_success() {
            return Err(LlmError::Api {
                status: response.status().as_u16(),
                message: "Health check failed".to_string(),
            });
        }

        Ok(())
    }
}

// OpenAI API types

#[derive(Debug, Serialize)]
struct OpenAIChatRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenAITool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

#[derive(Debug, Serialize)]
struct OpenAIMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAIToolCallRequest>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

impl From<&Message> for OpenAIMessage {
    fn from(msg: &Message) -> Self {
        let role = match msg.role {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::Tool => "tool",
        };

        OpenAIMessage {
            role: role.to_string(),
            content: if msg.content.is_empty() {
                None
            } else {
                Some(msg.content.clone())
            },
            tool_calls: msg.tool_calls.as_ref().map(|calls| {
                calls
                    .iter()
                    .map(|tc| OpenAIToolCallRequest {
                        id: tc.id.clone(),
                        r#type: "function".to_string(),
                        function: OpenAIFunctionCallRequest {
                            name: tc.name.clone(),
                            arguments: serde_json::to_string(&tc.arguments)
                                .unwrap_or_else(|_| "{}".to_string()),
                        },
                    })
                    .collect()
            }),
            tool_call_id: msg.tool_call_id.clone(),
            name: msg.tool_name.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
struct OpenAIToolCallRequest {
    id: String,
    r#type: String,
    function: OpenAIFunctionCallRequest,
}

#[derive(Debug, Serialize)]
struct OpenAIFunctionCallRequest {
    name: String,
    arguments: String,
}

#[derive(Debug, Serialize)]
struct OpenAITool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAIFunctionDef,
}

impl From<&ToolDefinition> for OpenAITool {
    fn from(def: &ToolDefinition) -> Self {
        OpenAITool {
            tool_type: "function".to_string(),
            function: OpenAIFunctionDef {
                name: def.name.clone(),
                description: def.description.clone(),
                parameters: def.parameters.clone(),
            },
        }
    }
}

#[derive(Debug, Serialize)]
struct OpenAIFunctionDef {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct OpenAIChatResponse {
    choices: Vec<OpenAIChoice>,
    usage: Option<OpenAIUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    message: OpenAIResponseMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponseMessage {
    content: Option<String>,
    tool_calls: Option<Vec<OpenAIToolCallResponse>>,
}

#[derive(Debug, Deserialize)]
struct OpenAIToolCallResponse {
    id: String,
    function: OpenAIFunctionCallResponse,
}

#[derive(Debug, Deserialize)]
struct OpenAIFunctionCallResponse {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_conversion() {
        let msg = Message::user("Hello");
        let openai: OpenAIMessage = (&msg).into();
        assert_eq!(openai.role, "user");
        assert_eq!(openai.content, Some("Hello".to_string()));

        let msg = Message::system("You are helpful");
        let openai: OpenAIMessage = (&msg).into();
        assert_eq!(openai.role, "system");
    }

    #[test]
    fn test_tool_definition_conversion() {
        let def = ToolDefinition {
            name: "search".to_string(),
            description: "Search the catalog".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"}
                },
                "required": ["query"]
            }),
        };

        let openai: OpenAITool = (&def).into();
        assert_eq!(openai.tool_type, "function");
        assert_eq!(openai.function.name, "search");
    }

    #[test]
    fn test_tool_message_conversion() {
        let msg = Message::tool_response("call_123", "search", "results here");
        let openai: OpenAIMessage = (&msg).into();
        assert_eq!(openai.role, "tool");
        assert_eq!(openai.tool_call_id, Some("call_123".to_string()));
        assert_eq!(openai.name, Some("search".to_string()));
    }
}
