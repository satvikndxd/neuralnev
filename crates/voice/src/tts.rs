//! TTS adapter trait + cancellable mock.
//!
//! The mock "speaks" by sleeping proportionally to the text length (~14 ms a
//! character, capped), and is interruptible at any point via the provided
//! cancellation token or the adapter-wide `stop()`.

use async_trait::async_trait;
use neuralnav_core::TtsError;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[async_trait]
pub trait TtsAdapter: Send + Sync {
    /// Speak `text`; must return early with `TtsError::Cancelled` if the
    /// signal fires mid-utterance.
    async fn speak(&self, text: String, signal: CancellationToken) -> Result<(), TtsError>;

    /// Hard-stop any in-flight utterance (barge-in).
    fn stop(&self);
}

pub struct MockTts {
    /// Cancels the *current* utterance when `stop` is called. Re-armed per call.
    current: std::sync::Mutex<CancellationToken>,
}

impl Default for MockTts {
    fn default() -> Self {
        Self { current: std::sync::Mutex::new(CancellationToken::new()) }
    }
}

#[async_trait]
impl TtsAdapter for MockTts {
    async fn speak(&self, text: String, signal: CancellationToken) -> Result<(), TtsError> {
        let own = CancellationToken::new();
        *self.current.lock().unwrap() = own.clone();

        let ms = (text.chars().count() as u64 * 14).clamp(80, 1500);
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(ms)) => Ok(()),
            _ = signal.cancelled() => Err(TtsError::Cancelled),
            _ = own.cancelled() => Err(TtsError::Cancelled),
        }
    }

    fn stop(&self) {
        self.current.lock().unwrap().cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn speak_completes_quickly() {
        let tts = MockTts::default();
        tts.speak("Opening Amazon.".into(), CancellationToken::new()).await.unwrap();
    }

    #[tokio::test]
    async fn speak_is_cancellable_by_signal() {
        let tts = MockTts::default();
        let token = CancellationToken::new();
        let t2 = token.clone();
        let handle = tokio::spawn(async move {
            tts.speak("a very long sentence that would take a while to speak out loud".into(), t2)
                .await
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        token.cancel();
        assert!(matches!(handle.await.unwrap(), Err(TtsError::Cancelled)));
    }

    #[tokio::test]
    async fn stop_interrupts_current_utterance() {
        let tts = std::sync::Arc::new(MockTts::default());
        let t = tts.clone();
        let handle = tokio::spawn(async move {
            t.speak("another long utterance for the stop test, still going".into(), CancellationToken::new())
                .await
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        tts.stop();
        assert!(matches!(handle.await.unwrap(), Err(TtsError::Cancelled)));
    }
}
