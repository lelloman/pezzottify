//! Ollama LLM provider implementation.

use super::provider::{CompletionOptions, LlmError, LlmProvider};
use super::types::{CompletionResponse, FinishReason, Message, MessageRole, TokenUsage, ToolCall};
use crate::agent::tools::ToolDefinition;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

/// Ollama LLM provider.
///
/// Connects to an Ollama server and uses its `/api/chat` endpoint
/// for completions with tool support.
pub struct OllamaProvider {
    client: Client,
    base_url: String,
    model: String,
}

impl OllamaProvider {
    /// Create a new Ollama provider.
    ///
    /// # Arguments
    /// * `base_url` - Base URL of the Ollama server (e.g., "http://localhost:11434").
    /// * `model` - Model to use (e.g., "llama3.1:8b").
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into(),
            model: model.into(),
        }
    }

    /// Convert our messages to Ollama's format.
    fn to_ollama_messages(messages: &[Message]) -> Vec<OllamaMessage> {
        messages.iter().map(|m| m.into()).collect()
    }

    /// Convert tool definitions to Ollama's format.
    fn to_ollama_tools(tools: &[ToolDefinition]) -> Vec<OllamaTool> {
        tools.iter().map(|t| t.into()).collect()
    }
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    fn name(&self) -> &str {
        "ollama"
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
        let url = format!("{}/api/chat", self.base_url);

        let request = OllamaChatRequest {
            model: self.model.clone(),
            messages: Self::to_ollama_messages(messages),
            tools: tools.map(Self::to_ollama_tools),
            stream: false,
            options: Some(OllamaOptions {
                temperature: Some(options.temperature),
                num_predict: options.max_tokens.map(|n| n as i32),
            }),
        };

        debug!(
            model = %self.model,
            message_count = messages.len(),
            has_tools = tools.is_some(),
            "Sending completion request to Ollama"
        );

        let response = self
            .client
            .post(&url)
            .json(&request)
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
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(LlmError::Api {
                status: status.as_u16(),
                message: body,
            });
        }

        let ollama_response: OllamaChatResponse = response.json().await.map_err(|e| {
            LlmError::InvalidResponse(format!("Failed to parse Ollama response: {}", e))
        })?;

        // Convert Ollama response to our format
        let has_tool_calls = ollama_response
            .message
            .tool_calls
            .as_ref()
            .map(|tc| !tc.is_empty())
            .unwrap_or(false);

        let tool_calls = ollama_response.message.tool_calls.map(|calls| {
            calls
                .into_iter()
                .enumerate()
                .map(|(i, tc)| ToolCall {
                    id: format!("call_{}", i),
                    name: tc.function.name,
                    arguments: tc.function.arguments,
                })
                .collect()
        });

        let message = Message {
            role: MessageRole::Assistant,
            content: ollama_response.message.content,
            tool_calls,
            tool_call_id: None,
            tool_name: None,
        };

        let finish_reason = if has_tool_calls {
            FinishReason::ToolCalls
        } else if ollama_response.done_reason.as_deref() == Some("length") {
            FinishReason::MaxTokens
        } else {
            FinishReason::Stop
        };

        let usage = Some(TokenUsage {
            prompt_tokens: ollama_response.prompt_eval_count.unwrap_or(0),
            completion_tokens: ollama_response.eval_count.unwrap_or(0),
            total_tokens: ollama_response.prompt_eval_count.unwrap_or(0)
                + ollama_response.eval_count.unwrap_or(0),
        });

        debug!(
            finish_reason = ?finish_reason,
            has_tool_calls = has_tool_calls,
            "Received completion response from Ollama"
        );

        Ok(CompletionResponse {
            message,
            finish_reason,
            usage,
        })
    }

    async fn health_check(&self) -> Result<(), LlmError> {
        let url = format!("{}/api/tags", self.base_url);

        let response = self
            .client
            .get(&url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| {
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

        // Optionally verify our model exists
        let tags: OllamaTagsResponse = response.json().await.map_err(|e| {
            LlmError::InvalidResponse(format!("Failed to parse tags response: {}", e))
        })?;

        let model_exists = tags.models.iter().any(|m| m.name == self.model);
        if !model_exists {
            warn!(
                model = %self.model,
                available_models = ?tags.models.iter().map(|m| &m.name).collect::<Vec<_>>(),
                "Configured model not found in Ollama"
            );
        }

        Ok(())
    }
}

// Ollama API types

#[derive(Debug, Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OllamaTool>>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
}

#[derive(Debug, Serialize)]
struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OllamaToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

impl From<&Message> for OllamaMessage {
    fn from(msg: &Message) -> Self {
        let role = match msg.role {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::Tool => "tool",
        };

        OllamaMessage {
            role: role.to_string(),
            content: msg.content.clone(),
            tool_calls: msg.tool_calls.as_ref().map(|calls| {
                calls
                    .iter()
                    .map(|tc| OllamaToolCall {
                        function: OllamaFunctionCall {
                            name: tc.name.clone(),
                            arguments: tc.arguments.clone(),
                        },
                    })
                    .collect()
            }),
            tool_call_id: msg.tool_call_id.clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaToolCall {
    function: OllamaFunctionCall,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaFunctionCall {
    name: String,
    arguments: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct OllamaTool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OllamaFunctionDef,
}

impl From<&ToolDefinition> for OllamaTool {
    fn from(def: &ToolDefinition) -> Self {
        OllamaTool {
            tool_type: "function".to_string(),
            function: OllamaFunctionDef {
                name: def.name.clone(),
                description: def.description.clone(),
                parameters: def.parameters.clone(),
            },
        }
    }
}

#[derive(Debug, Serialize)]
struct OllamaFunctionDef {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    message: OllamaMessage,
    #[allow(dead_code)]
    done: bool,
    #[serde(default)]
    done_reason: Option<String>,
    #[serde(default)]
    prompt_eval_count: Option<u32>,
    #[serde(default)]
    eval_count: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaModel {
    name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_conversion() {
        let msg = Message::user("Hello");
        let ollama: OllamaMessage = (&msg).into();
        assert_eq!(ollama.role, "user");
        assert_eq!(ollama.content, "Hello");

        let msg = Message::system("You are helpful");
        let ollama: OllamaMessage = (&msg).into();
        assert_eq!(ollama.role, "system");
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

        let ollama: OllamaTool = (&def).into();
        assert_eq!(ollama.tool_type, "function");
        assert_eq!(ollama.function.name, "search");
    }
}
