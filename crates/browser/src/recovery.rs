//! Failure → recovery routing. Every classified failure maps to a concrete
//! strategy; the executor emits `RecoveryAttempted` and applies it.

use neuralnav_core::FailureClass;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryStrategy {
    /// Retry the same action (optionally after a popup dismissal).
    Retry { max_attempts: u32 },
    /// Walk the selector ladder: role+name → visible text → CSS → vision.
    SelfHealSelector,
    /// Dismiss cookie banners / modals, then retry.
    DismissPopups,
    /// Pause and hand control to the human (auth walls, CAPTCHAs).
    PauseForHuman { reason: &'static str },
    /// Ask the user to clarify the command.
    AskClarification,
    /// Switch proxy / retry on network trouble.
    NetworkFailover,
    /// Nothing sensible left — abort the node safely.
    Abort,
}

pub fn strategy_for(class: FailureClass, attempts: u32) -> RecoveryStrategy {
    use FailureClass as F;
    match class {
        F::ElementNotFound if attempts < 2 => RecoveryStrategy::SelfHealSelector,
        F::ElementNotFound => RecoveryStrategy::Abort,
        F::PageTimeout if attempts < 2 => RecoveryStrategy::Retry { max_attempts: 2 },
        F::PageTimeout => RecoveryStrategy::Abort,
        F::PopupBlockingView => RecoveryStrategy::DismissPopups,
        F::AuthRequired => RecoveryStrategy::PauseForHuman { reason: "login required" },
        F::CaptchaRequired => RecoveryStrategy::PauseForHuman { reason: "CAPTCHA — never automated" },
        F::AmbiguousCommand => RecoveryStrategy::AskClarification,
        F::NetworkError if attempts < 2 => RecoveryStrategy::NetworkFailover,
        F::NetworkError => RecoveryStrategy::Abort,
        F::ActionVerificationFailed if attempts < 2 => RecoveryStrategy::Retry { max_attempts: 2 },
        F::ActionVerificationFailed => RecoveryStrategy::Abort,
        F::PolicyBlocked => RecoveryStrategy::Abort,
        F::Unknown if attempts < 1 => RecoveryStrategy::Retry { max_attempts: 1 },
        F::Unknown => RecoveryStrategy::Abort,
    }
}

pub fn strategy_label(s: &RecoveryStrategy) -> String {
    match s {
        RecoveryStrategy::Retry { max_attempts } => format!("retry (max {max_attempts})"),
        RecoveryStrategy::SelfHealSelector => {
            "self-heal selector: role+name → text → CSS → vision".into()
        }
        RecoveryStrategy::DismissPopups => "dismiss popups, then retry".into(),
        RecoveryStrategy::PauseForHuman { reason } => format!("pause for human ({reason})"),
        RecoveryStrategy::AskClarification => "ask user to clarify".into(),
        RecoveryStrategy::NetworkFailover => "proxy health-check + failover".into(),
        RecoveryStrategy::Abort => "safely abort node".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captcha_always_pauses_for_human() {
        for attempts in 0..5 {
            assert!(matches!(
                strategy_for(FailureClass::CaptchaRequired, attempts),
                RecoveryStrategy::PauseForHuman { .. }
            ));
        }
    }

    #[test]
    fn element_not_found_heals_then_aborts() {
        assert_eq!(strategy_for(FailureClass::ElementNotFound, 0), RecoveryStrategy::SelfHealSelector);
        assert_eq!(strategy_for(FailureClass::ElementNotFound, 1), RecoveryStrategy::SelfHealSelector);
        assert_eq!(strategy_for(FailureClass::ElementNotFound, 2), RecoveryStrategy::Abort);
    }

    #[test]
    fn verification_failure_retries_then_aborts() {
        assert!(matches!(
            strategy_for(FailureClass::ActionVerificationFailed, 0),
            RecoveryStrategy::Retry { .. }
        ));
        assert_eq!(strategy_for(FailureClass::ActionVerificationFailed, 3), RecoveryStrategy::Abort);
    }
}
