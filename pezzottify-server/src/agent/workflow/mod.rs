//! Workflow state machine and executor.
//!
//! This module provides the core workflow execution engine for agents,
//! including state management, step-by-step execution, and review handling.

mod executor;
mod state;

pub use executor::{Workflow, WorkflowError, WorkflowExecutor};
pub use state::{AgentAction, ReviewOption, WorkflowResult, WorkflowState};
