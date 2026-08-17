//! Verification helpers: never assume "dispatched == succeeded". These build
//! the concrete checks runtimes attach to every `ActionResult`.

use neuralnav_core::{PageState, VerificationCheck};

pub fn url_changed(before: &str, after: &str) -> VerificationCheck {
    if before != after {
        VerificationCheck::pass("URL changed", format!("{before} → {after}"))
    } else {
        VerificationCheck::fail("URL changed", format!("still {after}"))
    }
}

/// The navigation success criterion is "we arrived at the destination", not
/// "the URL differs from before" — re-navigating to a page you are already on
/// is still a successful navigation (this keeps demo replays honest).
pub fn url_arrived(before: &str, after: &str, target: &str) -> VerificationCheck {
    if after == target || after.starts_with(target) {
        let detail = if before == after {
            format!("re-navigated to {after}")
        } else {
            format!("{before} → {after}")
        };
        VerificationCheck::pass("URL is destination", detail)
    } else {
        VerificationCheck::fail("URL is destination", format!("expected {target}, got {after}"))
    }
}

pub fn result_count_reduced(before: u64, after: u64) -> VerificationCheck {
    if after < before {
        VerificationCheck::pass(
            "result count reduced",
            format!("{} → {}", group_thousands(before), group_thousands(after)),
        )
    } else {
        VerificationCheck::fail(
            "result count reduced",
            format!("{} → {}", group_thousands(before), group_thousands(after)),
        )
    }
}

pub fn min_candidates(count: usize, min: usize) -> VerificationCheck {
    if count >= min {
        VerificationCheck::pass(
            format!("at least {min} candidates"),
            format!("{count} extracted"),
        )
    } else {
        VerificationCheck::fail(
            format!("at least {min} candidates"),
            format!("only {count} extracted"),
        )
    }
}

/// Generic post-action sanity checks from a page-state snapshot.
pub fn page_healthy(state: &PageState) -> VerificationCheck {
    if state.loading {
        VerificationCheck::fail("page settled", "still loading")
    } else {
        VerificationCheck::pass("page settled", state.title.clone())
    }
}

fn group_thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out.chars().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_change_detected() {
        assert!(url_changed("about:blank", "https://a.example").passed);
        assert!(!url_changed("https://a.example", "https://a.example").passed);
    }

    #[test]
    fn url_arrival_accepts_renavigation_but_not_wrong_destination() {
        assert!(url_arrived("about:blank", "https://a.example", "https://a.example").passed);
        // Re-navigating to the same page is still a success (replay case).
        assert!(url_arrived("https://a.example", "https://a.example", "https://a.example").passed);
        assert!(!url_arrived("about:blank", "https://b.example", "https://a.example").passed);
    }

    #[test]
    fn count_reduction_detected() {
        let c = result_count_reduced(1204, 312);
        assert!(c.passed);
        assert!(c.detail.unwrap().contains("1,204 → 312"));
        assert!(!result_count_reduced(312, 312).passed);
    }

    #[test]
    fn candidate_floor_enforced() {
        assert!(min_candidates(4, 3).passed);
        assert!(!min_candidates(2, 3).passed);
    }
}
