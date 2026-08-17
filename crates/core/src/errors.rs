//! Error taxonomy for the whole workspace.

use crate::types::FailureClass;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GraphError {
    #[error("task graph has no nodes")]
    Empty,
    #[error("task node is missing an id")]
    MissingId,
    #[error("duplicate node id: {0}")]
    DuplicateId(String),
    #[error("node {node} depends on unknown node {dep}")]
    UnknownDependency { node: String, dep: String },
    #[error("dependency cycle involving node {0}")]
    Cycle(String),
    #[error("node {0} has an empty success_check")]
    MissingSuccessCheck(String),
}

#[derive(Debug, Error)]
pub enum PlannerError {
    #[error("planner produced invalid output: {0}")]
    InvalidOutput(String),
    #[error("planner transport error: {0}")]
    Transport(String),
    #[error("planner needs clarification: {0}")]
    NeedsClarification(String),
    #[error(transparent)]
    Graph(#[from] GraphError),
}

#[derive(Debug, Error)]
pub enum BrowserError {
    #[error("browser action failed ({class:?}): {message}")]
    Action { class: FailureClass, message: String },
    #[error("browser runtime unavailable: {0}")]
    Unavailable(String),
    #[error("action cancelled")]
    Cancelled,
}

impl BrowserError {
    pub fn class(&self) -> FailureClass {
        match self {
            Self::Action { class, .. } => *class,
            Self::Unavailable(_) => FailureClass::NetworkError,
            Self::Cancelled => FailureClass::Unknown,
        }
    }
}

#[derive(Debug, Error)]
pub enum AsrError {
    #[error("asr failed: {0}")]
    Failed(String),
    #[error("asr cancelled")]
    Cancelled,
}

#[derive(Debug, Error)]
pub enum TtsError {
    #[error("tts failed: {0}")]
    Failed(String),
    #[error("tts cancelled")]
    Cancelled,
}
