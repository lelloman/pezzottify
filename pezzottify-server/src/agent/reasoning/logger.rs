//! Reasoning step logger.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use uuid::Uuid;

/// Type of reasoning step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningStepType {
    /// Initial context provided to the agent.
    Context,
    /// Agent's internal reasoning/thinking.
    Thought,
    /// A tool call made by the agent.
    ToolCall,
    /// Result of a tool call.
    ToolResult,
    /// A decision made by the agent.
    Decision,
    /// A question posed for human review.
    ReviewQuestion,
    /// Answer received from human review.
    ReviewAnswer,
    /// Final action taken.
    Action,
    /// An error occurred.
    Error,
}

/// A single step in the agent's reasoning process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningStep {
    /// Unique identifier for this step.
    pub id: String,
    /// Step number within the workflow (0-indexed).
    pub step_number: u32,
    /// Unix timestamp (milliseconds).
    pub timestamp: i64,
    /// Type of step.
    pub step_type: ReasoningStepType,
    /// Human-readable content describing this step.
    pub content: String,
    /// Additional metadata (JSON).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    /// Duration of this step in milliseconds (if applicable).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
}

impl ReasoningStep {
    /// Create a new reasoning step.
    pub fn new(step_number: u32, step_type: ReasoningStepType, content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            step_number,
            timestamp: chrono::Utc::now().timestamp_millis(),
            step_type,
            content: content.into(),
            metadata: None,
            duration_ms: None,
        }
    }

    /// Add metadata to the step.
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Add duration to the step.
    pub fn with_duration_ms(mut self, duration_ms: i64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }
}

/// Logger for capturing agent reasoning steps.
///
/// This logger accumulates steps as the agent executes, providing
/// a complete trace of the agent's reasoning process.
pub struct ReasoningLogger {
    steps: Vec<ReasoningStep>,
    step_counter: AtomicU64,
    current_timer: Option<Instant>,
}

impl ReasoningLogger {
    /// Create a new reasoning logger.
    pub fn new() -> Self {
        Self {
            steps: Vec::new(),
            step_counter: AtomicU64::new(0),
            current_timer: None,
        }
    }

    /// Log a reasoning step.
    pub fn log(&mut self, step_type: ReasoningStepType, content: impl Into<String>) {
        let step_number = self.step_counter.fetch_add(1, Ordering::SeqCst) as u32;
        let step = ReasoningStep::new(step_number, step_type, content);
        self.steps.push(step);
    }

    /// Log a step with metadata.
    pub fn log_with_metadata(
        &mut self,
        step_type: ReasoningStepType,
        content: impl Into<String>,
        metadata: serde_json::Value,
    ) {
        let step_number = self.step_counter.fetch_add(1, Ordering::SeqCst) as u32;
        let step = ReasoningStep::new(step_number, step_type, content).with_metadata(metadata);
        self.steps.push(step);
    }

    /// Start a timer for the next step.
    pub fn start_timer(&mut self) {
        self.current_timer = Some(Instant::now());
    }

    /// Log a step and include the elapsed time since start_timer was called.
    pub fn log_with_elapsed(&mut self, step_type: ReasoningStepType, content: impl Into<String>) {
        let step_number = self.step_counter.fetch_add(1, Ordering::SeqCst) as u32;
        let mut step = ReasoningStep::new(step_number, step_type, content);

        if let Some(start) = self.current_timer.take() {
            step.duration_ms = Some(start.elapsed().as_millis() as i64);
        }

        self.steps.push(step);
    }

    /// Log a step with both metadata and elapsed time.
    pub fn log_with_metadata_and_elapsed(
        &mut self,
        step_type: ReasoningStepType,
        content: impl Into<String>,
        metadata: serde_json::Value,
    ) {
        let step_number = self.step_counter.fetch_add(1, Ordering::SeqCst) as u32;
        let mut step = ReasoningStep::new(step_number, step_type, content).with_metadata(metadata);

        if let Some(start) = self.current_timer.take() {
            step.duration_ms = Some(start.elapsed().as_millis() as i64);
        }

        self.steps.push(step);
    }

    /// Get all logged steps.
    pub fn steps(&self) -> &[ReasoningStep] {
        &self.steps
    }

    /// Get the latest step.
    pub fn latest(&self) -> Option<&ReasoningStep> {
        self.steps.last()
    }

    /// Get the number of steps logged.
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Check if any steps have been logged.
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Clear all logged steps.
    pub fn clear(&mut self) {
        self.steps.clear();
        self.step_counter.store(0, Ordering::SeqCst);
    }

    /// Take ownership of all steps, leaving the logger empty.
    pub fn take_steps(&mut self) -> Vec<ReasoningStep> {
        self.step_counter.store(0, Ordering::SeqCst);
        std::mem::take(&mut self.steps)
    }
}

impl Default for ReasoningLogger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reasoning_logger() {
        let mut logger = ReasoningLogger::new();

        logger.log(ReasoningStepType::Context, "Starting workflow");
        logger.log(ReasoningStepType::Thought, "Analyzing input...");
        logger.log_with_metadata(
            ReasoningStepType::ToolCall,
            "Calling search_catalog",
            serde_json::json!({"query": "Beatles"}),
        );

        assert_eq!(logger.len(), 3);
        assert_eq!(logger.steps()[0].step_number, 0);
        assert_eq!(logger.steps()[1].step_number, 1);
        assert_eq!(logger.steps()[2].step_number, 2);
        assert!(logger.steps()[2].metadata.is_some());
    }

    #[test]
    fn test_timing() {
        let mut logger = ReasoningLogger::new();

        logger.start_timer();
        std::thread::sleep(std::time::Duration::from_millis(10));
        logger.log_with_elapsed(ReasoningStepType::ToolCall, "Slow operation");

        assert!(logger.latest().unwrap().duration_ms.is_some());
        assert!(logger.latest().unwrap().duration_ms.unwrap() >= 10);
    }

    #[test]
    fn test_take_steps() {
        let mut logger = ReasoningLogger::new();
        logger.log(ReasoningStepType::Context, "Step 1");
        logger.log(ReasoningStepType::Thought, "Step 2");

        let steps = logger.take_steps();
        assert_eq!(steps.len(), 2);
        assert!(logger.is_empty());
    }
}
