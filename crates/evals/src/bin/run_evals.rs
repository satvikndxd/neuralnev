//! CLI: `cargo run -p neuralnav-evals --bin run-evals`

use neuralnav_evals::{default_dataset, run_planner_evals};
use neuralnav_planner::MockPlanner;

#[tokio::main]
async fn main() {
    let report = run_planner_evals(&MockPlanner, &default_dataset()).await;
    println!("── NeuralNav planner evals ──");
    for c in &report.cases {
        println!(
            "  [{}] intent {:>20} → {:<20} graph_valid={} {}ms  \"{}\"",
            if c.intent_correct && c.clarification_correct { "PASS" } else { "FAIL" },
            c.expected_intent,
            c.actual_intent,
            c.graph_valid,
            c.plan_latency_ms,
            c.transcript,
        );
    }
    println!("── summary ──");
    println!("  intent accuracy:        {:.1}%", report.intent_accuracy * 100.0);
    println!("  clarification accuracy: {:.1}%", report.clarification_accuracy * 100.0);
    println!("  graph validity:         {:.1}%", report.graph_validity_rate * 100.0);
    println!("  p50 plan latency:       {} ms", report.p50_plan_latency_ms);
    println!(
        "\n{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "intent_accuracy": report.intent_accuracy,
            "clarification_accuracy": report.clarification_accuracy,
            "graph_validity_rate": report.graph_validity_rate,
            "p50_plan_latency_ms": report.p50_plan_latency_ms,
        }))
        .unwrap()
    );
}
