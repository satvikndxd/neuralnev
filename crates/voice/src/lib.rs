//! neuralnav-voice — ASR and TTS adapter traits with deterministic mock
//! implementations, plus the barge-in controller that ties "user spoke over
//! us / pressed stop" to cancellation of both speech and execution.

pub mod asr;
pub mod barge_in;
pub mod tts;

pub use asr::{AsrAdapter, AudioInput, MockAsr};
pub use barge_in::BargeInController;
pub use tts::{MockTts, TtsAdapter};
