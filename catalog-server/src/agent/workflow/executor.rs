//! Workflow executor.

use super::state::{WorkflowResult, WorkflowState};
use crate::agent::llm::{CompletionOptions, LlmError, LlmProvider, Message};
use crate::agent::reasoning::{ReasoningLogger, ReasoningStepType};
use crate::agent::tools::{AgentToolRegistry, ToolContext, ToolError};
use std::sync::Arc;
use thiserror::Error;

/// Errors that can occur during workflow execution.
#[derive(Debug, Error)]
pub enum WorkflowError {
    #[error("LLM error: {0}")]
    Llm(#[from] LlmError),

    #[error("Tool error: {0}")]
    Tool(#[from] ToolError),

    #[error("Max iterations exceeded")]
    MaxIterationsExceeded,

    #[error("Invalid state transition: {0}")]
    InvalidStateTransition(String),

    #[error("Workflow failed: {0}")]
    Failed(String),
}

/// A workflow instance.
pub struct Workflow {
    /// Current state of the workflow.
    pub state: WorkflowState,
    /// Conversation history with the LLM.
    pub messages: Vec<Message>,
    /// Reasoning steps logged during execution.
    pub reasoning: ReasoningLogger,
    /// Number of iterations executed.
    pub iteration_count: usize,
    /// Arbitrary workflow data (for workflow-specific state).
    pub data: serde_json::Value,
}

impl Workflow {
    /// Create a new workflow with a system prompt.
    pub fn new(system_prompt: impl Into<String>) -> Self {
        Self {
            state: WorkflowState::Started,
            messages: vec![Message::system(system_prompt)],
            reasoning: ReasoningLogger::new(),
            iteration_count: 0,
            data: serde_json::Value::Null,
        }
    }

    /// Add workflow-specific data.
    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = data;
        self
    }

    /// Add a user message to the conversation.
    pub fn add_user_message(&mut self, content: impl Into<String>) {
        self.messages.push(Message::user(content));
    }

    /// Add an assistant message to the conversation.
    pub fn add_assistant_message(&mut self, content: impl Into<String>) {
        self.messages.push(Message::assistant(content));
    }

    /// Provide a review answer.
    pub fn provide_review_answer(&mut self, selected_option_id: &str) {
        if let WorkflowState::AwaitingReview { question, options } = &self.state {
            let selected = options.iter().find(|o| o.id == selected_option_id);
            let answer_text = selected.map(|o| o.label.as_str()).unwrap_or(selected_option_id);

            self.reasoning.log(
                ReasoningStepType::ReviewAnswer,
                format!("User selected: {}", answer_text),
            );

            // Add the review answer as a user message
            self.add_user_message(format!(
                "I've reviewed the options for: \"{}\"\nMy selection: {}",
                question, answer_text
            ));

            // Transition back to thinking state
            self.state = WorkflowState::Thinking;
        }
    }
}

/// Executes agent workflows step by step.
pub struct WorkflowExecutor {
    llm: Arc<dyn LlmProvider>,
    tools: Arc<AgentToolRegistry>,
    max_iterations: usize,
    completion_options: CompletionOptions,
}

impl WorkflowExecutor {
    /// Create a new workflow executor.
    pub fn new(
        llm: Arc<dyn LlmProvider>,
        tools: Arc<AgentToolRegistry>,
        max_iterations: usize,
    ) -> Self {
        Self {
            llm,
            tools,
            max_iterations,
            completion_options: CompletionOptions::default(),
        }
    }

    /// Set completion options.
    pub fn with_completion_options(mut self, options: CompletionOptions) -> Self {
        self.completion_options = options;
        self
    }

    /// Run one step of the workflow.
    pub async fn step(
        &self,
        workflow: &mut Workflow,
        ctx: &ToolContext,
    ) -> Result<(), WorkflowError> {
        workflow.iteration_count += 1;

        if workflow.iteration_count > self.max_iterations {
            workflow.state = WorkflowState::Failed {
                error: "Maximum iterations exceeded".to_string(),
                recoverable: false,
            };
            return Err(WorkflowError::MaxIterationsExceeded);
        }

        match &workflow.state {
            WorkflowState::Started => {
                workflow.reasoning.log(ReasoningStepType::Context, "Starting workflow");
                workflow.state = WorkflowState::Thinking;
            }

            WorkflowState::Thinking => {
                workflow
                    .reasoning
                    .log(ReasoningStepType::Thought, "Requesting LLM completion");
                workflow.reasoning.start_timer();

                let tool_defs = self.tools.definitions();
                let tools = if tool_defs.is_empty() {
                    None
                } else {
                    Some(tool_defs.as_slice())
                };

                let response = self
                    .llm
                    .complete(&workflow.messages, tools, &self.completion_options)
                    .await?;

                workflow.reasoning.log_with_elapsed(
                    ReasoningStepType::Thought,
                    format!("LLM response: {}", truncate(&response.message.content, 200)),
                );

                // Add assistant message to conversation
                workflow.messages.push(response.message.clone());

                // Check if we have tool calls
                if let Some(tool_calls) = &response.message.tool_calls {
                    if !tool_calls.is_empty() {
                        workflow.state = WorkflowState::ExecutingTools {
                            tool_calls: tool_calls.clone(),
                        };
                        return Ok(());
                    }
                }

                // No tool calls - workflow is complete
                workflow.state = WorkflowState::Completed {
                    result: WorkflowResult::success(response.message.content),
                };
            }

            WorkflowState::ExecutingTools { tool_calls } => {
                let tool_calls = tool_calls.clone();

                for tool_call in tool_calls {
                    workflow.reasoning.log_with_metadata(
                        ReasoningStepType::ToolCall,
                        format!("Calling tool: {}", tool_call.name),
                        serde_json::json!({
                            "tool": tool_call.name,
                            "arguments": tool_call.arguments
                        }),
                    );
                    workflow.reasoning.start_timer();

                    let result = self
                        .tools
                        .execute(&tool_call.name, tool_call.arguments.clone(), ctx)
                        .await;

                    let (content, is_error) = match result {
                        Ok(value) => (serde_json::to_string_pretty(&value).unwrap_or_default(), false),
                        Err(e) => (format!("Error: {}", e), true),
                    };

                    workflow.reasoning.log_with_elapsed(
                        ReasoningStepType::ToolResult,
                        format!(
                            "Tool {} returned: {}",
                            tool_call.name,
                            truncate(&content, 200)
                        ),
                    );

                    // Add tool response to conversation
                    workflow.messages.push(Message::tool_response(
                        &tool_call.id,
                        &tool_call.name,
                        &content,
                    ));

                    if is_error {
                        // Continue anyway - the LLM will see the error and can decide how to proceed
                    }
                }

                // Go back to thinking to process tool results
                workflow.state = WorkflowState::Thinking;
            }

            WorkflowState::AwaitingReview { .. } => {
                // Cannot proceed without review answer
                return Err(WorkflowError::InvalidStateTransition(
                    "Workflow is awaiting review".to_string(),
                ));
            }

            WorkflowState::ReadyToExecute { .. } => {
                // This state is set by workflow-specific code
                // The executor doesn't handle execution - that's up to the caller
                return Err(WorkflowError::InvalidStateTransition(
                    "Workflow is ready to execute - action must be handled by caller".to_string(),
                ));
            }

            WorkflowState::Executing => {
                // Same as above
                return Err(WorkflowError::InvalidStateTransition(
                    "Workflow is executing - must be handled by caller".to_string(),
                ));
            }

            WorkflowState::Completed { .. } | WorkflowState::Failed { .. } => {
                return Err(WorkflowError::InvalidStateTransition(
                    "Workflow already in terminal state".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Run workflow until it blocks (needs review), completes, or fails.
    pub async fn run_until_blocked(
        &self,
        workflow: &mut Workflow,
        ctx: &ToolContext,
    ) -> Result<(), WorkflowError> {
        while workflow.state.can_continue() {
            self.step(workflow, ctx).await?;
        }
        Ok(())
    }
}

/// Truncate a string to a maximum length.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::llm::MessageRole;
    use crate::agent::workflow::state::ReviewOption;

    #[test]
    fn test_workflow_creation() {
        let workflow = Workflow::new("You are a helpful assistant.");

        assert!(matches!(workflow.state, WorkflowState::Started));
        assert_eq!(workflow.messages.len(), 1);
        assert_eq!(workflow.messages[0].role, MessageRole::System);
    }

    #[test]
    fn test_workflow_messages() {
        let mut workflow = Workflow::new("System");
        workflow.add_user_message("Hello");
        workflow.add_assistant_message("Hi there");

        assert_eq!(workflow.messages.len(), 3);
        assert_eq!(workflow.messages[1].role, MessageRole::User);
        assert_eq!(workflow.messages[2].role, MessageRole::Assistant);
    }

    #[test]
    fn test_review_answer() {
        let mut workflow = Workflow::new("System");
        workflow.state = WorkflowState::AwaitingReview {
            question: "Which album?".to_string(),
            options: vec![
                ReviewOption::new("opt1", "Abbey Road"),
                ReviewOption::new("opt2", "Let It Be"),
            ],
        };

        workflow.provide_review_answer("opt1");

        assert!(matches!(workflow.state, WorkflowState::Thinking));
        // Should have logged the review answer
        assert!(!workflow.reasoning.is_empty());
    }
}
