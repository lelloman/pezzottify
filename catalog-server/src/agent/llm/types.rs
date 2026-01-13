//! Common types for LLM interactions.

use serde::{Deserialize, Serialize};

/// Role of a message in a conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

/// A message in a conversation with an LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    /// Tool calls requested by the assistant (if role is Assistant).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    /// ID of the tool call this message responds to (if role is Tool).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Name of the tool (if role is Tool).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
}

impl Message {
    /// Create a system message.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
            tool_name: None,
        }
    }

    /// Create a user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
            tool_name: None,
        }
    }

    /// Create an assistant message.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
            tool_name: None,
        }
    }

    /// Create an assistant message with tool calls.
    pub fn assistant_with_tools(content: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            tool_calls: Some(tool_calls),
            tool_call_id: None,
            tool_name: None,
        }
    }

    /// Create a tool response message.
    pub fn tool_response(
        tool_call_id: impl Into<String>,
        tool_name: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            role: MessageRole::Tool,
            content: content.into(),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
            tool_name: Some(tool_name.into()),
        }
    }
}

/// A tool call requested by the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique identifier for this tool call.
    pub id: String,
    /// Name of the tool to call.
    pub name: String,
    /// Arguments to pass to the tool (as JSON).
    pub arguments: serde_json::Value,
}

/// Response from an LLM completion request.
#[derive(Debug, Clone)]
pub struct CompletionResponse {
    /// The assistant's response message.
    pub message: Message,
    /// Why the completion finished.
    pub finish_reason: FinishReason,
    /// Token usage information (if available).
    pub usage: Option<TokenUsage>,
}

/// Why an LLM completion finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinishReason {
    /// Natural end of response.
    Stop,
    /// Model wants to call tools.
    ToolCalls,
    /// Hit the maximum token limit.
    MaxTokens,
    /// An error occurred.
    Error,
}

/// Token usage information.
#[derive(Debug, Clone, Copy)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_constructors() {
        let sys = Message::system("You are helpful");
        assert_eq!(sys.role, MessageRole::System);
        assert_eq!(sys.content, "You are helpful");

        let user = Message::user("Hello");
        assert_eq!(user.role, MessageRole::User);

        let asst = Message::assistant("Hi there");
        assert_eq!(asst.role, MessageRole::Assistant);
        assert!(asst.tool_calls.is_none());

        let tool_calls = vec![ToolCall {
            id: "call_1".to_string(),
            name: "search".to_string(),
            arguments: serde_json::json!({"query": "test"}),
        }];
        let asst_tools = Message::assistant_with_tools("", tool_calls);
        assert!(asst_tools.tool_calls.is_some());
        assert_eq!(asst_tools.tool_calls.as_ref().unwrap().len(), 1);

        let tool_resp = Message::tool_response("call_1", "search", "results here");
        assert_eq!(tool_resp.role, MessageRole::Tool);
        assert_eq!(tool_resp.tool_call_id, Some("call_1".to_string()));
    }
}
