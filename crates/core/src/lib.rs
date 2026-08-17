//! neuralnav-core — shared, serde-validated types for the whole system.
//!
//! Everything the planner emits, the browser executes, the verifier checks
//! and the frontend renders flows through the types in this crate. There is
//! deliberately **no** free-form "run this JavaScript" escape hatch: actions
//! are a closed enum, validated at deserialization time.

pub mod errors;
pub mod events;
pub mod schema;
pub mod task_graph;
pub mod types;

pub use errors::*;
pub use events::TraceEvent;
pub use task_graph::{TaskGraph, TaskNode, TaskStatus};
pub use types::*;
