//! Validation of untrusted planner output (e.g. an LLM response) into a
//! [`TaskGraph`]. Deserialization through the closed serde enums *is* the
//! schema check — anything outside the action vocabulary fails here, before
//! it can reach a browser runtime.

use crate::errors::PlannerError;
use crate::task_graph::TaskGraph;

/// Parse and validate a raw JSON string (possibly wrapped in markdown fences,
/// as LLMs love to do) into a validated `TaskGraph`.
pub fn parse_task_graph(raw: &str) -> Result<TaskGraph, PlannerError> {
    let cleaned = strip_code_fences(raw);
    let graph: TaskGraph = serde_json::from_str(cleaned)
        .map_err(|e| PlannerError::InvalidOutput(format!("json: {e}")))?;
    graph.validate()?;
    Ok(graph)
}

/// LLMs frequently wrap JSON in ```json fences; tolerate that one deviation.
fn strip_code_fences(raw: &str) -> &str {
    let t = raw.trim();
    let t = t.strip_prefix("```json").or_else(|| t.strip_prefix("```")).unwrap_or(t);
    let t = t.strip_suffix("```").unwrap_or(t);
    t.trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"{
      "goal": "open example",
      "nodes": [
        {
          "id": "n1",
          "title": "Navigate",
          "action": { "type": "navigate", "url": "https://example.com" },
          "depends_on": [],
          "success_check": "url changed"
        }
      ]
    }"#;

    #[test]
    fn parses_valid_graph() {
        let g = parse_task_graph(VALID).unwrap();
        assert_eq!(g.nodes.len(), 1);
    }

    #[test]
    fn parses_fenced_graph() {
        let fenced = format!("```json\n{VALID}\n```");
        assert!(parse_task_graph(&fenced).is_ok());
    }

    #[test]
    fn rejects_unknown_action() {
        let bad = r#"{
          "goal": "g",
          "nodes": [{
            "id": "n1", "title": "Evil",
            "action": { "type": "run_js", "code": "fetch('/steal')" },
            "success_check": "x"
          }]
        }"#;
        assert!(parse_task_graph(bad).is_err());
    }

    #[test]
    fn rejects_structurally_invalid_graph() {
        let bad = r#"{
          "goal": "g",
          "nodes": [{
            "id": "n1", "title": "A",
            "action": { "type": "go_back" },
            "depends_on": ["missing"],
            "success_check": "x"
          }]
        }"#;
        assert!(parse_task_graph(bad).is_err());
    }
}
