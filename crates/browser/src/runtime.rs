//! The `BrowserRuntime` trait — the only door between planned actions and a
//! browser. Implementations receive *structured* actions and a cancellation
//! token; they never receive code.

use async_trait::async_trait;
use neuralnav_core::{ActionResult, BrowserError, NeuralNavAction, PageState};
use tokio_util::sync::CancellationToken;

#[async_trait]
pub trait BrowserRuntime: Send + Sync {
    /// Execute one structured action. Must observe `signal` and return
    /// `Err(BrowserError::Cancelled)` promptly when it fires.
    async fn execute(
        &self,
        action: NeuralNavAction,
        signal: CancellationToken,
    ) -> Result<ActionResult, BrowserError>;

    /// Snapshot of the current page (accessibility-tree-first summary).
    async fn page_state(&self) -> Result<PageState, BrowserError>;

    /// Tear down / abort anything in flight.
    async fn stop(&self) -> Result<(), BrowserError>;
}
