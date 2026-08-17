//! The `Planner` trait plus a light intent summary used for trace events.

use async_trait::async_trait;
use neuralnav_core::{PlannerError, PlannerInput, TaskGraph};

#[async_trait]
pub trait Planner: Send + Sync {
    async fn plan(&self, input: PlannerInput) -> Result<TaskGraph, PlannerError>;

    /// Cheap intent classification for the `IntentParsed` trace event.
    /// Default derives it from the transcript with simple heuristics.
    fn classify_intent(&self, transcript: &str) -> IntentSummary {
        IntentSummary::heuristic(transcript)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct IntentSummary {
    pub intent: String,
    pub confidence: f32,
    pub ambiguous: bool,
}

impl IntentSummary {
    pub fn heuristic(transcript: &str) -> Self {
        let t = transcript.to_lowercase();
        let words = t.split_whitespace().count();

        // Referring expressions with no antecedent → ambiguous.
        let ambiguous_refs = ["the second one", "that one", "this one", "it again"];
        if ambiguous_refs.iter().any(|r| t.contains(r)) {
            return Self { intent: "ambiguous_reference".into(), confidence: 0.55, ambiguous: true };
        }
        if words <= 1 {
            return Self { intent: "unclear".into(), confidence: 0.4, ambiguous: true };
        }

        let compound = ["and", "then", "under", "with", "compare", "filter"]
            .iter()
            .filter(|k| t.contains(*k))
            .count();
        if compound >= 2 || words > 8 {
            Self { intent: "composite_web_task".into(), confidence: 0.94, ambiguous: false }
        } else if t.starts_with("open") || t.starts_with("go to") || t.starts_with("visit") {
            Self { intent: "navigate".into(), confidence: 0.97, ambiguous: false }
        } else if t.starts_with("search") || t.starts_with("find") || t.starts_with("look up") {
            Self { intent: "search".into(), confidence: 0.92, ambiguous: false }
        } else {
            Self { intent: "web_task".into(), confidence: 0.8, ambiguous: false }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_command_is_high_confidence_composite() {
        let s = IntentSummary::heuristic(
            "Open Amazon and find a mechanical keyboard under 5,000 rupees with good reviews.",
        );
        assert_eq!(s.intent, "composite_web_task");
        assert!(s.confidence > 0.9);
        assert!(!s.ambiguous);
    }

    #[test]
    fn dangling_reference_is_ambiguous() {
        let s = IntentSummary::heuristic("open the second one");
        assert!(s.ambiguous);
    }
}
