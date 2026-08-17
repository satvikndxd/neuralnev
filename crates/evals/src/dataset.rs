//! Labelled evaluation commands: expected intent + whether the planner
//! should ask for clarification.

#[derive(Debug, Clone)]
pub struct EvalCase {
    pub transcript: &'static str,
    pub expected_intent: &'static str,
    pub expect_clarification: bool,
}

pub fn default_dataset() -> Vec<EvalCase> {
    vec![
        EvalCase {
            transcript:
                "Open Amazon and find a mechanical keyboard under 5,000 rupees with good reviews.",
            expected_intent: "composite_web_task",
            expect_clarification: false,
        },
        EvalCase {
            transcript: "open youtube",
            expected_intent: "navigate",
            expect_clarification: false,
        },
        EvalCase {
            transcript: "open google",
            expected_intent: "navigate",
            expect_clarification: false,
        },
        EvalCase {
            transcript: "search for transformers",
            expected_intent: "search",
            expect_clarification: false,
        },
        EvalCase {
            transcript: "find the best rust web framework",
            expected_intent: "search",
            expect_clarification: false,
        },
        EvalCase {
            transcript: "open the second one",
            expected_intent: "ambiguous_reference",
            expect_clarification: true,
        },
        EvalCase {
            transcript: "go to wikipedia",
            expected_intent: "navigate",
            expect_clarification: false,
        },
        EvalCase {
            transcript:
                "find and compare the top 3 wireless earbuds under 3000 with warranty details",
            expected_intent: "composite_web_task",
            expect_clarification: false,
        },
        EvalCase {
            transcript: "visit github",
            expected_intent: "navigate",
            expect_clarification: false,
        },
        EvalCase {
            transcript: "stop",
            expected_intent: "unclear",
            expect_clarification: true,
        },
    ]
}
