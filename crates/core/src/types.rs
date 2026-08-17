//! Core action / state / result types. Structured actions only — the planner
//! can never emit arbitrary code, only members of [`NeuralNavAction`].

use serde::{Deserialize, Serialize};

/// Every action the agent is capable of dispatching to a browser runtime
/// (or to the voice layer). A closed set, tagged on `type`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NeuralNavAction {
    Navigate {
        url: String,
    },
    Click {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        selector: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        role: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
    },
    Type {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        selector: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        role: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        text: String,
    },
    Scroll {
        direction: ScrollDirection,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        amount: Option<u32>,
    },
    Wait {
        ms: u64,
    },
    Extract {
        fields: Vec<String>,
    },
    GoBack,
    Reload,
    Speak {
        message: String,
    },
    AskUser {
        question: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        options: Option<Vec<String>>,
    },
    ConfirmSensitiveAction {
        description: String,
    },
}

impl NeuralNavAction {
    /// Short human label used in logs and the UI.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Navigate { .. } => "navigate",
            Self::Click { .. } => "click",
            Self::Type { .. } => "type",
            Self::Scroll { .. } => "scroll",
            Self::Wait { .. } => "wait",
            Self::Extract { .. } => "extract",
            Self::GoBack => "go_back",
            Self::Reload => "reload",
            Self::Speak { .. } => "speak",
            Self::AskUser { .. } => "ask_user",
            Self::ConfirmSensitiveAction { .. } => "confirm_sensitive_action",
        }
    }

    /// Does this action mutate page/browser state (vs. read-only)?
    pub fn is_mutating(&self) -> bool {
        matches!(
            self,
            Self::Click { .. } | Self::Type { .. } | Self::ConfirmSensitiveAction { .. }
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrollDirection {
    Up,
    Down,
}

/// A summarized view of the current page, built accessibility-tree-first.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PageState {
    pub url: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accessibility_summary: Option<String>,
    #[serde(default)]
    pub interactive_elements: Vec<InteractiveElement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_count: Option<u64>,
    #[serde(default)]
    pub loading: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InteractiveElement {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
}

/// Result of executing one action, including its verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub ok: bool,
    pub action: NeuralNavAction,
    pub verification: VerificationResult,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_state: Option<PageState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_class: Option<FailureClass>,
    pub duration_ms: u64,
    /// Structured data produced by `Extract` actions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extracted: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub passed: bool,
    pub checks: Vec<VerificationCheck>,
}

impl VerificationResult {
    pub fn from_checks(checks: Vec<VerificationCheck>) -> Self {
        let passed = !checks.is_empty() && checks.iter().all(|c| c.passed);
        Self { passed, checks }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationCheck {
    pub label: String,
    pub passed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl VerificationCheck {
    pub fn pass(label: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            passed: true,
            detail: Some(detail.into()),
        }
    }
    pub fn fail(label: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            passed: false,
            detail: Some(detail.into()),
        }
    }
}

/// Every failure is classified so recovery can route on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureClass {
    ElementNotFound,
    PageTimeout,
    PopupBlockingView,
    AuthRequired,
    CaptchaRequired,
    AmbiguousCommand,
    NetworkError,
    ActionVerificationFailed,
    PolicyBlocked,
    Unknown,
}

/// Input handed to a [`Planner`](https://docs.rs — see planner crate) impl.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlannerInput {
    pub transcript: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_state: Option<PageState>,
    #[serde(default)]
    pub permission_level: PermissionLevel,
    #[serde(default)]
    pub prior_actions: Vec<NeuralNavAction>,
}

/// Guardrail permission levels (see guardrails crate for the policy engine).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PermissionLevel {
    /// Level 1 — read-only browsing.
    ReadOnly,
    /// Level 2 — may click/type; sensitive actions need confirmation.
    #[default]
    Interactive,
    /// Level 3 — sensitive actions permitted, each behind explicit confirmation.
    Restricted,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_serde_roundtrip_is_tagged_snake_case() {
        let a = NeuralNavAction::Type {
            selector: None,
            role: Some("textbox".into()),
            name: Some("Search".into()),
            text: "mechanical keyboard".into(),
        };
        let json = serde_json::to_value(&a).unwrap();
        assert_eq!(json["type"], "type");
        assert_eq!(json["role"], "textbox");
        assert!(json.get("selector").is_none(), "None fields are omitted");
        let back: NeuralNavAction = serde_json::from_value(json).unwrap();
        assert_eq!(back, a);
    }

    #[test]
    fn unknown_action_type_is_rejected() {
        let raw = serde_json::json!({ "type": "eval_js", "code": "alert(1)" });
        assert!(serde_json::from_value::<NeuralNavAction>(raw).is_err());
    }

    #[test]
    fn verification_result_requires_all_checks_to_pass() {
        let ok = VerificationResult::from_checks(vec![
            VerificationCheck::pass("a", ""),
            VerificationCheck::pass("b", ""),
        ]);
        assert!(ok.passed);
        let bad = VerificationResult::from_checks(vec![
            VerificationCheck::pass("a", ""),
            VerificationCheck::fail("b", "nope"),
        ]);
        assert!(!bad.passed);
        let empty = VerificationResult::from_checks(vec![]);
        assert!(!empty.passed, "no checks means not verified");
    }
}
