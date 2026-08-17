//! Metric aggregation for eval runs.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct CaseResult {
    pub transcript: String,
    pub expected_intent: String,
    pub actual_intent: String,
    pub intent_correct: bool,
    pub clarification_correct: bool,
    pub graph_valid: bool,
    pub plan_latency_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct EvalReport {
    pub cases: Vec<CaseResult>,
    pub intent_accuracy: f64,
    pub clarification_accuracy: f64,
    pub graph_validity_rate: f64,
    pub p50_plan_latency_ms: u64,
}

impl EvalReport {
    pub fn from_cases(cases: Vec<CaseResult>) -> Self {
        let n = cases.len().max(1) as f64;
        let intent_accuracy =
            cases.iter().filter(|c| c.intent_correct).count() as f64 / n;
        let clarification_accuracy =
            cases.iter().filter(|c| c.clarification_correct).count() as f64 / n;
        let graph_validity_rate =
            cases.iter().filter(|c| c.graph_valid).count() as f64 / n;
        let mut lat: Vec<u64> = cases.iter().map(|c| c.plan_latency_ms).collect();
        lat.sort_unstable();
        let p50_plan_latency_ms = lat.get(lat.len() / 2).copied().unwrap_or(0);
        Self {
            cases,
            intent_accuracy,
            clarification_accuracy,
            graph_validity_rate,
            p50_plan_latency_ms,
        }
    }
}
