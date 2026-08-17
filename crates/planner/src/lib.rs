//! neuralnav-planner — turns a transcript (+ page state + constraints) into a
//! validated [`TaskGraph`]. Two implementations:
//! - [`mock_planner::MockPlanner`] — deterministic, keyword-driven, powers
//!   the no-keys demo.
//! - [`gemini_planner::GeminiPlanner`] — calls the Gemini API, validates the
//!   JSON through `neuralnav_core::schema`, retries once, and falls back to
//!   the mock on any persistent failure.

pub mod gemini_planner;
pub mod mock_planner;
pub mod planner;
pub mod prompts;

pub use gemini_planner::GeminiPlanner;
pub use mock_planner::MockPlanner;
pub use planner::{IntentSummary, Planner};
