//! Structured trace events — the observability backbone. Every important
//! state change is one of these, streamed to the frontend over SSE.

use crate::task_graph::TaskGraph;
use crate::types::{ActionResult, FailureClass, NeuralNavAction};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum TraceEvent {
    SessionStarted {
        session_id: String,
        timestamp: i64,
    },
    VoicePartialTranscript {
        text: String,
    },
    VoiceFinalTranscript {
        text: String,
    },
    IntentParsed {
        intent: String,
        confidence: f32,
    },
    PlanCreated {
        graph: TaskGraph,
    },
    ActionDispatched {
        node_id: String,
        action: NeuralNavAction,
    },
    ActionVerified {
        node_id: String,
        result: ActionResult,
    },
    ActionFailed {
        node_id: String,
        error_class: FailureClass,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    RecoveryAttempted {
        node_id: String,
        strategy: String,
    },
    PolicyDecision {
        node_id: String,
        decision: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    UserConfirmationRequested {
        question: String,
    },
    UserConfirmationResolved {
        approved: bool,
    },
    UserStopped {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    TtsSpoken {
        message: String,
    },
    SessionCompleted {
        success: bool,
    },
}

impl TraceEvent {
    /// Event name used as the SSE `event:` field.
    pub fn name(&self) -> &'static str {
        match self {
            Self::SessionStarted { .. } => "session_started",
            Self::VoicePartialTranscript { .. } => "voice_partial_transcript",
            Self::VoiceFinalTranscript { .. } => "voice_final_transcript",
            Self::IntentParsed { .. } => "intent_parsed",
            Self::PlanCreated { .. } => "plan_created",
            Self::ActionDispatched { .. } => "action_dispatched",
            Self::ActionVerified { .. } => "action_verified",
            Self::ActionFailed { .. } => "action_failed",
            Self::RecoveryAttempted { .. } => "recovery_attempted",
            Self::PolicyDecision { .. } => "policy_decision",
            Self::UserConfirmationRequested { .. } => "user_confirmation_requested",
            Self::UserConfirmationResolved { .. } => "user_confirmation_resolved",
            Self::UserStopped { .. } => "user_stopped",
            Self::TtsSpoken { .. } => "tts_spoken",
            Self::SessionCompleted { .. } => "session_completed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_serialize_with_tag() {
        let e = TraceEvent::IntentParsed { intent: "web_task".into(), confidence: 0.94 };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["event"], "intent_parsed");
        assert_eq!(v["intent"], "web_task");
    }
}
