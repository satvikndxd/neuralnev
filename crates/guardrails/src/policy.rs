//! Policy engine: given (action, permission level) → Allowed / Blocked /
//! ConfirmationRequired. Deterministic and conservative: anything that looks
//! like payment/checkout/account-mutation requires explicit confirmation,
//! CAPTCHAs are never automated.

use neuralnav_core::{NeuralNavAction, PermissionLevel};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum PolicyDecision {
    Allowed,
    Blocked { reason: String },
    ConfirmationRequired { description: String },
}

/// Keywords that mark an action as sensitive/irreversible regardless of level.
const SENSITIVE_KEYWORDS: &[&str] = &[
    "checkout", "payment", "pay now", "buy now", "place order", "purchase",
    "delete account", "delete", "remove account", "send message", "send",
    "download", "unsubscribe", "transfer", "confirm order",
];

#[derive(Debug, Clone, Default)]
pub struct PolicyEngine;

impl PolicyEngine {
    pub fn new() -> Self {
        Self
    }

    pub fn evaluate(&self, action: &NeuralNavAction, level: PermissionLevel) -> PolicyDecision {
        use NeuralNavAction as A;

        // CAPTCHA policy is absolute: we never automate solving one. There is
        // no CAPTCHA action in the vocabulary, but a planner could try to
        // click one — catch it by name.
        if let Some(label) = action_label(action) {
            if label.contains("captcha") {
                return PolicyDecision::Blocked {
                    reason: "CAPTCHAs are never automated; pausing for the human".into(),
                };
            }
        }

        // Explicit sensitive-action node always requires confirmation.
        if let A::ConfirmSensitiveAction { description } = action {
            return PolicyDecision::ConfirmationRequired {
                description: description.clone(),
            };
        }

        let sensitive = is_sensitive(action);

        match level {
            PermissionLevel::ReadOnly => match action {
                A::Navigate { .. }
                | A::Scroll { .. }
                | A::Wait { .. }
                | A::Extract { .. }
                | A::GoBack
                | A::Reload
                | A::Speak { .. }
                | A::AskUser { .. } => PolicyDecision::Allowed,
                A::Type { .. } | A::Click { .. } | A::ConfirmSensitiveAction { .. } => {
                    PolicyDecision::Blocked {
                        reason: format!(
                            "'{}' is a mutating action; permission level is read-only",
                            action.kind()
                        ),
                    }
                }
            },
            PermissionLevel::Interactive | PermissionLevel::Restricted => {
                if sensitive {
                    PolicyDecision::ConfirmationRequired {
                        description: sensitive_description(action),
                    }
                } else {
                    PolicyDecision::Allowed
                }
            }
        }
    }
}

/// Lowercased text surface of an action for keyword scanning.
fn action_label(action: &NeuralNavAction) -> Option<String> {
    use NeuralNavAction as A;
    let joined = match action {
        A::Click { selector, role, name, text } => [selector, role, name, text]
            .iter()
            .filter_map(|o| o.as_deref())
            .collect::<Vec<_>>()
            .join(" "),
        A::Type { selector, role, name, text } => {
            let mut parts: Vec<&str> = [selector, role, name]
                .iter()
                .filter_map(|o| o.as_deref())
                .collect();
            parts.push(text);
            parts.join(" ")
        }
        A::Navigate { url } => url.clone(),
        A::ConfirmSensitiveAction { description } => description.clone(),
        _ => return None,
    };
    Some(joined.to_lowercase())
}

fn is_sensitive(action: &NeuralNavAction) -> bool {
    match action_label(action) {
        Some(label) => SENSITIVE_KEYWORDS.iter().any(|k| label.contains(k)),
        None => false,
    }
}

fn sensitive_description(action: &NeuralNavAction) -> String {
    format!(
        "The next step ('{}') looks sensitive (payment / message / irreversible). Proceed?",
        action.kind()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use neuralnav_core::NeuralNavAction as A;

    fn click(name: &str) -> A {
        A::Click { selector: None, role: Some("button".into()), name: Some(name.into()), text: None }
    }

    #[test]
    fn read_only_blocks_typing_and_clicking() {
        let p = PolicyEngine::new();
        assert!(matches!(
            p.evaluate(&click("Search"), PermissionLevel::ReadOnly),
            PolicyDecision::Blocked { .. }
        ));
        assert!(matches!(
            p.evaluate(
                &A::Type { selector: None, role: None, name: None, text: "hi".into() },
                PermissionLevel::ReadOnly
            ),
            PolicyDecision::Blocked { .. }
        ));
    }

    #[test]
    fn read_only_allows_navigation_and_extraction() {
        let p = PolicyEngine::new();
        assert_eq!(
            p.evaluate(&A::Navigate { url: "https://example.com".into() }, PermissionLevel::ReadOnly),
            PolicyDecision::Allowed
        );
        assert_eq!(
            p.evaluate(&A::Extract { fields: vec!["title".into()] }, PermissionLevel::ReadOnly),
            PolicyDecision::Allowed
        );
    }

    #[test]
    fn interactive_allows_plain_click_but_confirms_checkout() {
        let p = PolicyEngine::new();
        assert_eq!(p.evaluate(&click("Search"), PermissionLevel::Interactive), PolicyDecision::Allowed);
        assert!(matches!(
            p.evaluate(&click("Proceed to Checkout"), PermissionLevel::Interactive),
            PolicyDecision::ConfirmationRequired { .. }
        ));
        assert!(matches!(
            p.evaluate(&click("Buy Now"), PermissionLevel::Restricted),
            PolicyDecision::ConfirmationRequired { .. }
        ));
    }

    #[test]
    fn explicit_sensitive_action_always_requires_confirmation() {
        let p = PolicyEngine::new();
        for level in [PermissionLevel::ReadOnly, PermissionLevel::Interactive, PermissionLevel::Restricted] {
            assert!(matches!(
                p.evaluate(
                    &A::ConfirmSensitiveAction { description: "pay ₹4,299".into() },
                    level
                ),
                PolicyDecision::ConfirmationRequired { .. }
            ));
        }
    }

    #[test]
    fn captcha_click_is_blocked_at_every_level() {
        let p = PolicyEngine::new();
        for level in [PermissionLevel::ReadOnly, PermissionLevel::Interactive, PermissionLevel::Restricted] {
            assert!(matches!(
                p.evaluate(&click("I am not a robot (CAPTCHA)"), level),
                PolicyDecision::Blocked { .. }
            ));
        }
    }
}
