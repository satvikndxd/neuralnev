//! neuralnav-server library surface (the binary in `main.rs` is a thin
//! wrapper). Exposed so integration tests can drive the orchestrator
//! end-to-end without a network socket.

pub mod events;
pub mod routes;
pub mod session;
pub mod state;

use neuralnav_guardrails::PolicyEngine;
use state::{AppState, Config, SessionState};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

/// Build a fully-mocked AppState (used by tests and available for embedding).
pub fn mock_app_state(config: Config) -> Arc<AppState> {
    use neuralnav_browser::MockBrowserRuntime;
    use neuralnav_planner::MockPlanner;
    use neuralnav_voice::{MockAsr, MockTts};

    let (events_tx, _) = broadcast::channel(256);
    Arc::new(AppState {
        config,
        events_tx,
        session: Mutex::new(SessionState::default()),
        planner: Arc::new(MockPlanner),
        browser: Arc::new(MockBrowserRuntime::default()),
        asr: Arc::new(MockAsr::default()),
        tts: Arc::new(MockTts::default()),
        policy: PolicyEngine::new(),
    })
}
