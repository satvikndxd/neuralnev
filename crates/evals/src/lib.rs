//! neuralnav-evals — a small evaluation harness for the planner layer.
//! Measures intent accuracy and clarification behavior over a labelled
//! command dataset. Run with `cargo run -p neuralnav-evals --bin run-evals`.

pub mod dataset;
pub mod metrics;
pub mod runner;

pub use dataset::{default_dataset, EvalCase};
pub use metrics::EvalReport;
pub use runner::run_planner_evals;
