//! neuralnav-guardrails — the policy engine that sits between the planner
//! and the browser runtime. Every action passes through
//! [`policy::PolicyEngine::evaluate`] before dispatch.

pub mod permissions;
pub mod policy;

pub use permissions::describe_level;
pub use policy::{PolicyDecision, PolicyEngine};
