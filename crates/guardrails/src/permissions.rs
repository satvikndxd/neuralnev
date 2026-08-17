//! Human-readable descriptions of the permission levels (used by the UI and
//! the planner prompt).

use neuralnav_core::PermissionLevel;

pub fn describe_level(level: PermissionLevel) -> &'static str {
    match level {
        PermissionLevel::ReadOnly => {
            "Level 1 — Read-only: navigate, scroll, extract, go back, reload. \
             No typing, clicking, form submission, purchases or deletions."
        }
        PermissionLevel::Interactive => {
            "Level 2 — Interactive: navigate, click, type, extract. Payments, \
             message sending, account changes and deletions require confirmation."
        }
        PermissionLevel::Restricted => {
            "Level 3 — Restricted: checkout, payment, account changes, sending \
             messages, downloads and other irreversible actions each require \
             explicit spoken/clicked confirmation."
        }
    }
}
