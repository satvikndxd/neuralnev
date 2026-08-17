//! Eval runner: pushes every dataset case through a planner and scores it.

use crate::dataset::EvalCase;
use crate::metrics::{CaseResult, EvalReport};
use neuralnav_core::{NeuralNavAction, PlannerInput};
use neuralnav_planner::Planner;
use std::time::Instant;

pub async fn run_planner_evals(planner: &dyn Planner, dataset: &[EvalCase]) -> EvalReport {
    let mut results = Vec::with_capacity(dataset.len());
    for case in dataset {
        let started = Instant::now();
        let intent = planner.classify_intent(case.transcript);
        let plan = planner
            .plan(PlannerInput { transcript: case.transcript.into(), ..Default::default() })
            .await;
        let plan_latency_ms = started.elapsed().as_millis() as u64;

        let (graph_valid, asked_clarification) = match &plan {
            Ok(g) => (
                g.validate().is_ok(),
                g.nodes
                    .iter()
                    .any(|n| matches!(n.action, NeuralNavAction::AskUser { .. })),
            ),
            Err(_) => (false, false),
        };

        results.push(CaseResult {
            transcript: case.transcript.into(),
            expected_intent: case.expected_intent.into(),
            actual_intent: intent.intent.clone(),
            intent_correct: intent.intent == case.expected_intent,
            clarification_correct: asked_clarification == case.expect_clarification,
            graph_valid,
            plan_latency_ms,
        });
    }
    EvalReport::from_cases(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset::default_dataset;
    use neuralnav_planner::MockPlanner;

    #[tokio::test]
    async fn mock_planner_clears_quality_bars() {
        let report = run_planner_evals(&MockPlanner, &default_dataset()).await;
        assert!(
            report.intent_accuracy >= 0.9,
            "intent accuracy {} below bar",
            report.intent_accuracy
        );
        assert!(
            report.clarification_accuracy >= 0.9,
            "clarification accuracy {} below bar",
            report.clarification_accuracy
        );
        assert_eq!(report.graph_validity_rate, 1.0, "every plan must validate");
    }
}
