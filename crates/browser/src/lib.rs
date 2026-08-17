//! neuralnav-browser — the `BrowserRuntime` abstraction and its
//! implementations.
//!
//! Architecture rule: we do NOT force pure-Rust browser automation.
//! - [`mock_runtime::MockBrowserRuntime`] — fully implemented, deterministic,
//!   powers the no-keys demo.
//! - [`playwright_sidecar::PlaywrightSidecarRuntime`] — the recommended real
//!   runtime: a small Node worker speaking JSON-lines over stdio. Structured
//!   commands only; the worker refuses arbitrary JavaScript.
//! - A pure-Rust CDP runtime is reserved behind the `cdp` feature flag but
//!   intentionally unimplemented (see ASSUMPTIONS.md).

pub mod adblock;
#[cfg(feature = "cdp")]
pub mod cdp_runtime;
pub mod mock_runtime;
pub mod page_state;
pub mod playwright_sidecar;
pub mod proxy;
pub mod recovery;
pub mod runtime;
pub mod verifier;

pub use mock_runtime::MockBrowserRuntime;
pub use playwright_sidecar::PlaywrightSidecarRuntime;
pub use runtime::BrowserRuntime;
