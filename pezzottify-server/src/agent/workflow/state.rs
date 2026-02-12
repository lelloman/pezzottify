//! Workflow state definitions.

use crate::agent::llm::ToolCall;
use serde::{Deserialize, Serialize};

/// State of an agent workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum WorkflowState {
    /// Workflow just started.
    Started,

    /// Agent is thinking (waiting for LLM response).
    Thinking,

    /// Agent has requested tool calls.
    ExecutingTools { tool_calls: Vec<ToolCall> },

    /// Waiting for human review.
    AwaitingReview {
        question: String,
        options: Vec<ReviewOption>,
    },

    /// Agent has decided on an action, ready to execute.
    ReadyToExecute { action: AgentAction },

    /// Executing the final action.
    Executing,

    /// Workflow completed successfully.
    Completed { result: WorkflowResult },

    /// Workflow failed.
    Failed { error: String, recoverable: bool },
}

impl WorkflowState {
    /// Check if the workflow is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            WorkflowState::Completed { .. } | WorkflowState::Failed { .. }
        )
    }

    /// Check if the workflow is blocked waiting for human input.
    pub fn is_blocked(&self) -> bool {
        matches!(self, WorkflowState::AwaitingReview { .. })
    }

    /// Check if the workflow can continue without human input.
    pub fn can_continue(&self) -> bool {
        !self.is_terminal() && !self.is_blocked()
    }
}

/// An option presented to the user during review.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewOption {
    /// Unique identifier for this option.
    pub id: String,
    /// Human-readable label.
    pub label: String,
    /// Additional description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Confidence score (0.0 - 1.0) if applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,
    /// Additional data associated with this option.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl ReviewOption {
    /// Create a new review option.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: None,
            confidence: None,
            data: None,
        }
    }

    /// Add a description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Add a confidence score.
    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = Some(confidence);
        self
    }

    /// Add associated data.
    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }
}

/// Action to be executed by the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum AgentAction {
    /// Match an uploaded file to a catalog track.
    MatchToTrack {
        file_id: String,
        track_id: String,
        confidence: f32,
    },

    /// Fulfill a pending download request.
    FulfillDownloadRequest {
        file_id: String,
        request_id: String,
        track_id: String,
    },

    /// Reject the upload (no suitable match).
    Reject { reason: String },

    /// Custom action (for extensibility).
    Custom {
        action_type: String,
        data: serde_json::Value,
    },
}

/// Result of a completed workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowResult {
    /// Whether the workflow succeeded.
    pub success: bool,
    /// Summary of what was accomplished.
    pub summary: String,
    /// Actions that were taken.
    #[serde(default)]
    pub actions_taken: Vec<AgentAction>,
    /// Additional result data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl WorkflowResult {
    /// Create a successful result.
    pub fn success(summary: impl Into<String>) -> Self {
        Self {
            success: true,
            summary: summary.into(),
            actions_taken: Vec::new(),
            data: None,
        }
    }

    /// Create a failure result.
    pub fn failure(summary: impl Into<String>) -> Self {
        Self {
            success: false,
            summary: summary.into(),
            actions_taken: Vec::new(),
            data: None,
        }
    }

    /// Add actions taken.
    pub fn with_actions(mut self, actions: Vec<AgentAction>) -> Self {
        self.actions_taken = actions;
        self
    }

    /// Add result data.
    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_state_predicates() {
        assert!(!WorkflowState::Started.is_terminal());
        assert!(!WorkflowState::Started.is_blocked());
        assert!(WorkflowState::Started.can_continue());

        let completed = WorkflowState::Completed {
            result: WorkflowResult::success("Done"),
        };
        assert!(completed.is_terminal());
        assert!(!completed.is_blocked());
        assert!(!completed.can_continue());

        let awaiting = WorkflowState::AwaitingReview {
            question: "Which album?".to_string(),
            options: vec![],
        };
        assert!(!awaiting.is_terminal());
        assert!(awaiting.is_blocked());
        assert!(!awaiting.can_continue());
    }

    #[test]
    fn test_review_option_builder() {
        let opt = ReviewOption::new("opt1", "Option 1")
            .with_description("First option")
            .with_confidence(0.95)
            .with_data(serde_json::json!({"track_id": "abc123"}));

        assert_eq!(opt.id, "opt1");
        assert_eq!(opt.label, "Option 1");
        assert_eq!(opt.confidence, Some(0.95));
    }
}
