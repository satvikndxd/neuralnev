//! ASR adapter trait + deterministic mock.
//!
//! The real system would stream microphone audio through VAD into a streaming
//! ASR engine. In this repo, audio never leaves the browser: the frontend is
//! expected to use the Web Speech API (or send text), and `MockAsr` simulates
//! a finished transcription for demo mode. A real server-side ASR adapter
//! (Whisper/Deepgram) would implement the same trait.

use async_trait::async_trait;
use neuralnav_core::AsrError;

/// Input to an ASR adapter. For the mock, `hint` carries the demo utterance;
/// a real adapter would carry PCM/opus bytes.
#[derive(Debug, Clone, Default)]
pub struct AudioInput {
    pub bytes: Vec<u8>,
    pub hint: Option<String>,
}

#[async_trait]
pub trait AsrAdapter: Send + Sync {
    async fn transcribe(&self, input: AudioInput) -> Result<String, AsrError>;
}

/// Deterministic mock: "recognizes" the hint (or the canonical demo command)
/// after a short, realistic delay.
pub struct MockAsr {
    pub default_utterance: String,
}

impl Default for MockAsr {
    fn default() -> Self {
        Self {
            default_utterance:
                "Open Amazon and find a mechanical keyboard under 5,000 rupees with good reviews."
                    .to_string(),
        }
    }
}

#[async_trait]
impl AsrAdapter for MockAsr {
    async fn transcribe(&self, input: AudioInput) -> Result<String, AsrError> {
        tokio::time::sleep(std::time::Duration::from_millis(120)).await;
        Ok(input.hint.unwrap_or_else(|| self.default_utterance.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_asr_returns_hint_or_default() {
        let asr = MockAsr::default();
        let t = asr
            .transcribe(AudioInput { bytes: vec![], hint: Some("open google".into()) })
            .await
            .unwrap();
        assert_eq!(t, "open google");
        let t2 = asr.transcribe(AudioInput::default()).await.unwrap();
        assert!(t2.contains("mechanical keyboard"));
    }
}
