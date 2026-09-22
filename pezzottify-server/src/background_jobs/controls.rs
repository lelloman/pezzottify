use super::job::JobResourceClass;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub(crate) const PAUSE_STATE_KEY: &str = "background_jobs.pause_state.v1";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobPauseState {
    #[serde(default)]
    pub global_paused: bool,
    #[serde(default)]
    pub paused_resource_classes: BTreeSet<JobResourceClass>,
    #[serde(default)]
    pub paused_jobs: BTreeSet<String>,
}

impl JobPauseState {
    pub(crate) fn is_paused(&self, job_id: &str, resource_class: JobResourceClass) -> bool {
        simple_server::task_policies::PauseState {
            global: self.global_paused,
            pools: self
                .paused_resource_classes
                .iter()
                .map(|class| class.as_str().to_string())
                .collect(),
            jobs: self.paused_jobs.clone(),
        }
        .is_paused(job_id, Some(resource_class.as_str()))
    }
}

#[derive(Debug, Clone)]
pub(crate) enum JobPauseScope {
    Global,
    ResourceClass(JobResourceClass),
    Job(String),
}

impl JobPauseScope {
    pub(crate) fn matches(&self, job_id: &str, resource_class: JobResourceClass) -> bool {
        match self {
            Self::Global => true,
            Self::ResourceClass(expected) => *expected == resource_class,
            Self::Job(expected) => expected == job_id,
        }
    }
}
