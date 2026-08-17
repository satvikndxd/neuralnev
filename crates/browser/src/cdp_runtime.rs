//! Pure-Rust CDP runtime — reserved, intentionally unimplemented.
//!
//! Evaluated options (2025/2026): `chromiumoxide` and `headless_chrome`.
//! Both lag Chrome's CDP surface for accessibility-tree queries and
//! role/name locators, which NeuralNav's selector ladder depends on. Per the
//! project's architectural rule ("do not force pure Rust browser automation
//! if it becomes brittle"), the sidecar runtime remains the supported real
//! backend. This module exists so the `cdp` feature flag compiles and the
//! trait surface is pinned down for a future implementation.

use crate::runtime::BrowserRuntime;
use async_trait::async_trait;
use neuralnav_core::{ActionResult, BrowserError, NeuralNavAction, PageState};
use tokio_util::sync::CancellationToken;

pub struct CdpBrowserRuntime;

#[async_trait]
impl BrowserRuntime for CdpBrowserRuntime {
    async fn execute(
        &self,
        _action: NeuralNavAction,
        _signal: CancellationToken,
    ) -> Result<ActionResult, BrowserError> {
        Err(BrowserError::Unavailable(
            "CDP runtime is a reserved feature flag; use the Playwright sidecar (see ASSUMPTIONS.md)".into(),
        ))
    }

    async fn page_state(&self) -> Result<PageState, BrowserError> {
        Err(BrowserError::Unavailable("CDP runtime not implemented".into()))
    }

    async fn stop(&self) -> Result<(), BrowserError> {
        Ok(())
    }
}
