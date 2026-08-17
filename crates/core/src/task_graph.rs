//! Task graph: the planner's output. A DAG of atomic actions with per-node
//! success criteria. Validated before execution (unique ids, resolvable
//! dependencies, no cycles).

use crate::errors::GraphError;
use crate::types::NeuralNavAction;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    #[default]
    Pending,
    Running,
    Success,
    Failed,
    Skipped,
    WaitingUser,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskNode {
    pub id: String,
    pub title: String,
    pub action: NeuralNavAction,
    #[serde(default)]
    pub depends_on: Vec<String>,
    pub success_check: String,
    #[serde(default)]
    pub status: TaskStatus,
    #[serde(default)]
    pub attempts: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

impl TaskNode {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        action: NeuralNavAction,
        depends_on: Vec<&str>,
        success_check: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            action,
            depends_on: depends_on.into_iter().map(String::from).collect(),
            success_check: success_check.into(),
            status: TaskStatus::Pending,
            attempts: 0,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGraph {
    pub goal: String,
    pub nodes: Vec<TaskNode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

impl TaskGraph {
    /// Validate structural invariants: non-empty, unique ids, deps resolve,
    /// success checks present, and the DAG is acyclic.
    pub fn validate(&self) -> Result<(), GraphError> {
        if self.nodes.is_empty() {
            return Err(GraphError::Empty);
        }
        let mut seen = HashSet::new();
        for n in &self.nodes {
            if n.id.trim().is_empty() {
                return Err(GraphError::MissingId);
            }
            if !seen.insert(n.id.as_str()) {
                return Err(GraphError::DuplicateId(n.id.clone()));
            }
            if n.success_check.trim().is_empty() {
                return Err(GraphError::MissingSuccessCheck(n.id.clone()));
            }
        }
        for n in &self.nodes {
            for d in &n.depends_on {
                if !seen.contains(d.as_str()) {
                    return Err(GraphError::UnknownDependency {
                        node: n.id.clone(),
                        dep: d.clone(),
                    });
                }
                if d == &n.id {
                    return Err(GraphError::Cycle(n.id.clone()));
                }
            }
        }
        self.topological_order().map(|_| ())
    }

    /// Kahn's algorithm; also serves as the executor's scheduling order.
    pub fn topological_order(&self) -> Result<Vec<String>, GraphError> {
        let mut indegree: HashMap<&str, usize> =
            self.nodes.iter().map(|n| (n.id.as_str(), 0)).collect();
        let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();
        for n in &self.nodes {
            for d in &n.depends_on {
                *indegree.entry(n.id.as_str()).or_insert(0) += 1;
                dependents.entry(d.as_str()).or_default().push(n.id.as_str());
            }
        }
        // Stable order: keep declaration order among ready nodes.
        let mut order = Vec::with_capacity(self.nodes.len());
        let mut ready: Vec<&str> = self
            .nodes
            .iter()
            .filter(|n| indegree[n.id.as_str()] == 0)
            .map(|n| n.id.as_str())
            .collect();
        while let Some(id) = ready.first().copied() {
            ready.remove(0);
            order.push(id.to_string());
            if let Some(deps) = dependents.get(id) {
                for d in deps {
                    let e = indegree.get_mut(d).unwrap();
                    *e -= 1;
                    if *e == 0 {
                        ready.push(d);
                    }
                }
            }
        }
        if order.len() != self.nodes.len() {
            let stuck = self
                .nodes
                .iter()
                .find(|n| !order.contains(&n.id))
                .map(|n| n.id.clone())
                .unwrap_or_default();
            return Err(GraphError::Cycle(stuck));
        }
        Ok(order)
    }

    pub fn node(&self, id: &str) -> Option<&TaskNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn node_mut(&mut self, id: &str) -> Option<&mut TaskNode> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::NeuralNavAction as A;

    fn nav(id: &str, deps: Vec<&str>) -> TaskNode {
        TaskNode::new(
            id,
            id.to_uppercase(),
            A::Navigate { url: "https://example.com".into() },
            deps,
            "url changed",
        )
    }

    #[test]
    fn valid_chain_passes_and_orders() {
        let g = TaskGraph {
            goal: "g".into(),
            nodes: vec![nav("a", vec![]), nav("b", vec!["a"]), nav("c", vec!["b"])],
            metadata: None,
        };
        g.validate().unwrap();
        assert_eq!(g.topological_order().unwrap(), vec!["a", "b", "c"]);
    }

    #[test]
    fn duplicate_ids_rejected() {
        let g = TaskGraph {
            goal: "g".into(),
            nodes: vec![nav("a", vec![]), nav("a", vec![])],
            metadata: None,
        };
        assert!(matches!(g.validate(), Err(GraphError::DuplicateId(_))));
    }

    #[test]
    fn unknown_dependency_rejected() {
        let g = TaskGraph {
            goal: "g".into(),
            nodes: vec![nav("a", vec!["ghost"])],
            metadata: None,
        };
        assert!(matches!(g.validate(), Err(GraphError::UnknownDependency { .. })));
    }

    #[test]
    fn cycle_rejected() {
        let g = TaskGraph {
            goal: "g".into(),
            nodes: vec![nav("a", vec!["b"]), nav("b", vec!["a"])],
            metadata: None,
        };
        assert!(matches!(g.validate(), Err(GraphError::Cycle(_))));
    }

    #[test]
    fn missing_success_check_rejected() {
        let mut n = nav("a", vec![]);
        n.success_check = "  ".into();
        let g = TaskGraph { goal: "g".into(), nodes: vec![n], metadata: None };
        assert!(matches!(g.validate(), Err(GraphError::MissingSuccessCheck(_))));
    }
}
