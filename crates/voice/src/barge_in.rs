//! Barge-in: the user's voice (or the Stop button) always wins.
//!
//! One controller owns the run-wide [`CancellationToken`]. Anything long-
//! running — TTS, browser actions, waits — selects on this token. Triggering
//! barge-in cancels the token *and* hard-stops TTS so speech halts mid-word.

use crate::tts::TtsAdapter;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct BargeInController {
    token: CancellationToken,
    tts: Arc<dyn TtsAdapter>,
}

impl BargeInController {
    pub fn new(tts: Arc<dyn TtsAdapter>) -> Self {
        Self { token: CancellationToken::new(), tts }
    }

    /// Token every cancellable operation should select on.
    pub fn token(&self) -> CancellationToken {
        self.token.clone()
    }

    /// User interrupted: stop speech now, cancel everything downstream.
    pub fn trigger(&self) {
        self.tts.stop();
        self.token.cancel();
    }

    pub fn is_triggered(&self) -> bool {
        self.token.is_cancelled()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tts::MockTts;

    #[tokio::test]
    async fn trigger_cancels_token() {
        let ctl = BargeInController::new(Arc::new(MockTts::default()));
        let token = ctl.token();
        assert!(!token.is_cancelled());
        ctl.trigger();
        assert!(token.is_cancelled());
        assert!(ctl.is_triggered());
    }
}
