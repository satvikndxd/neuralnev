//! Deterministic mock planner. Keyword-driven task-graph construction:
//! - the canonical shopping demo gets the full 5-node graph from the spec;
//! - simple "open <site>" commands get a navigate(+extract) graph;
//! - ambiguous commands come back as an `AskUser` clarification node.

use crate::planner::{IntentSummary, Planner};
use async_trait::async_trait;
use neuralnav_core::{
    NeuralNavAction as A, PlannerError, PlannerInput, TaskGraph, TaskNode,
};
use serde_json::json;

#[derive(Debug, Default, Clone)]
pub struct MockPlanner;

impl MockPlanner {
    pub fn demo_graph(goal: &str) -> TaskGraph {
        TaskGraph {
            goal: goal.into(),
            nodes: vec![
                TaskNode::new(
                    "navigate",
                    "Navigate",
                    A::Navigate { url: "https://www.amazon.in".into() },
                    vec![],
                    "URL changed to Amazon home or search page",
                ),
                TaskNode::new(
                    "search",
                    "Search",
                    A::Type {
                        selector: None,
                        role: Some("textbox".into()),
                        name: Some("Search".into()),
                        text: "mechanical keyboard".into(),
                    },
                    vec!["navigate"],
                    "Search results container visible",
                ),
                TaskNode::new(
                    "filter",
                    "Filter",
                    A::Click {
                        selector: None,
                        role: Some("link".into()),
                        name: Some("Under ₹5,000".into()),
                        text: None,
                    },
                    vec!["search"],
                    "Result count reduced",
                ),
                TaskNode::new(
                    "rank",
                    "Rank",
                    A::Extract {
                        fields: vec![
                            "title".into(),
                            "price".into(),
                            "rating".into(),
                            "reviews".into(),
                        ],
                    },
                    vec!["filter"],
                    "At least 3 candidate products extracted",
                ),
                TaskNode::new(
                    "choose",
                    "Choose best",
                    A::Click {
                        selector: None,
                        role: Some("link".into()),
                        name: Some("Best rated candidate".into()),
                        text: None,
                    },
                    vec!["rank"],
                    "Product detail page opened",
                ),
            ],
            metadata: Some(json!({
                "planner": "mock",
                "success_criteria": "product page for a keyboard under ₹5,000 with rating ≥ 4.5",
                "tts": {
                    "navigate": "Opening Amazon.",
                    "search": "Searching for keyboards.",
                    "filter": "Filtering under five thousand rupees.",
                    "rank": "Ranking by rating.",
                    "choose": "Done. Top pick — Cosmic Byte, four thousand two hundred ninety-nine rupees, rated four point six stars."
                }
            })),
        }
    }

    fn clarification_graph(goal: &str, question: &str, options: Vec<String>) -> TaskGraph {
        TaskGraph {
            goal: goal.into(),
            nodes: vec![TaskNode::new(
                "clarify",
                "Ask for clarification",
                A::AskUser { question: question.into(), options: Some(options) },
                vec![],
                "User provided a disambiguating answer",
            )],
            metadata: Some(json!({ "planner": "mock", "ambiguous": true })),
        }
    }

    fn navigate_graph(goal: &str, url: String, site: &str) -> TaskGraph {
        TaskGraph {
            goal: goal.into(),
            nodes: vec![
                TaskNode::new(
                    "navigate",
                    "Navigate",
                    A::Navigate { url },
                    vec![],
                    format!("URL changed to {site}"),
                ),
                TaskNode::new(
                    "survey",
                    "Survey page",
                    A::Extract { fields: vec!["title".into(), "headings".into(), "links".into()] },
                    vec!["navigate"],
                    "Page summary extracted",
                ),
            ],
            metadata: Some(json!({
                "planner": "mock",
                "tts": { "navigate": format!("Opening {site}."), "survey": "Here's what I can see." }
            })),
        }
    }
}

fn known_site_url(t: &str) -> Option<(&'static str, &'static str)> {
    const SITES: &[(&str, &str, &str)] = &[
        ("amazon", "https://www.amazon.in", "Amazon"),
        ("google", "https://www.google.com", "Google"),
        ("youtube", "https://www.youtube.com", "YouTube"),
        ("wikipedia", "https://www.wikipedia.org", "Wikipedia"),
        ("github", "https://github.com", "GitHub"),
    ];
    SITES
        .iter()
        .find(|(k, _, _)| t.contains(k))
        .map(|(_, url, name)| (*url, *name))
}

#[async_trait]
impl Planner for MockPlanner {
    async fn plan(&self, input: PlannerInput) -> Result<TaskGraph, PlannerError> {
        let t = input.transcript.to_lowercase();
        let intent = IntentSummary::heuristic(&input.transcript);

        let graph = if intent.ambiguous {
            Self::clarification_graph(
                &input.transcript,
                "That was ambiguous — which one do you mean?",
                vec![
                    "second search result".into(),
                    "second tab".into(),
                    "second card on the page".into(),
                ],
            )
        } else if t.contains("keyboard") || (t.contains("amazon") && t.contains("find")) {
            Self::demo_graph(&input.transcript)
        } else if let Some((url, site)) = known_site_url(&t) {
            Self::navigate_graph(&input.transcript, url.into(), site)
        } else {
            // Generic: treat as a search task on the current or default engine.
            let query = input.transcript.trim_end_matches('.').to_string();
            TaskGraph {
                goal: input.transcript.clone(),
                nodes: vec![
                    TaskNode::new(
                        "navigate",
                        "Navigate",
                        A::Navigate { url: "https://www.google.com".into() },
                        vec![],
                        "URL changed to search engine",
                    ),
                    TaskNode::new(
                        "search",
                        "Search",
                        A::Type {
                            selector: None,
                            role: Some("textbox".into()),
                            name: Some("Search".into()),
                            text: query,
                        },
                        vec!["navigate"],
                        "Search results container visible",
                    ),
                    TaskNode::new(
                        "survey",
                        "Survey results",
                        A::Extract { fields: vec!["title".into(), "url".into(), "snippet".into()] },
                        vec!["search"],
                        "Top results extracted",
                    ),
                ],
                metadata: Some(json!({ "planner": "mock" })),
            }
        };

        graph.validate()?;
        Ok(graph)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEMO: &str =
        "Open Amazon and find a mechanical keyboard under 5,000 rupees with good reviews.";

    #[tokio::test]
    async fn demo_command_yields_five_node_graph() {
        let g = MockPlanner
            .plan(PlannerInput { transcript: DEMO.into(), ..Default::default() })
            .await
            .unwrap();
        let ids: Vec<_> = g.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["navigate", "search", "filter", "rank", "choose"]);
        g.validate().unwrap();
        assert_eq!(
            g.topological_order().unwrap(),
            vec!["navigate", "search", "filter", "rank", "choose"]
        );
        // Every node carries a success check per the spec.
        assert!(g.nodes.iter().all(|n| !n.success_check.is_empty()));
    }

    #[tokio::test]
    async fn ambiguous_command_asks_for_clarification() {
        let g = MockPlanner
            .plan(PlannerInput { transcript: "open the second one".into(), ..Default::default() })
            .await
            .unwrap();
        assert_eq!(g.nodes.len(), 1);
        assert!(matches!(g.nodes[0].action, A::AskUser { .. }));
    }

    #[tokio::test]
    async fn simple_open_yields_navigate_graph() {
        let g = MockPlanner
            .plan(PlannerInput { transcript: "open youtube".into(), ..Default::default() })
            .await
            .unwrap();
        assert!(matches!(&g.nodes[0].action, A::Navigate { url } if url.contains("youtube")));
    }

    #[tokio::test]
    async fn generic_command_yields_search_graph() {
        let g = MockPlanner
            .plan(PlannerInput {
                transcript: "find the best rust web framework".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(g.nodes.iter().any(|n| matches!(n.action, A::Type { .. })));
        g.validate().unwrap();
    }
}
