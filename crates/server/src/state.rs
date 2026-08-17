//! Application + session state. One active session at a time (see
//! ASSUMPTIONS.md); all mutation happens under a short-lived std::sync::Mutex
//! that is never held across an await point.

use neuralnav_browser::BrowserRuntime;
use neuralnav_core::{PermissionLevel, TaskGraph, TraceEvent};
use neuralnav_guardrails::PolicyEngine;
use neuralnav_planner::Planner;
use neuralnav_voice::{AsrAdapter, TtsAdapter};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, oneshot};
use tokio_util::sync::CancellationToken;

pub const EVENT_HISTORY_CAP: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Idle,
    Running,
    WaitingConfirmation,
    Completed,
    Stopped,
}

#[derive(Default)]
pub struct SessionState {
    pub session_id: Option<String>,
    pub status: Option<RunStatus>,
    pub transcript: Option<String>,
    pub graph: Option<TaskGraph>,
    pub cancel: Option<CancellationToken>,
    pub confirm_tx: Option<oneshot::Sender<bool>>,
    pub history: Vec<TraceEvent>,
    pub last_success: Option<bool>,
}

impl SessionState {
    pub fn status(&self) -> RunStatus {
        self.status.unwrap_or(RunStatus::Idle)
    }

    pub fn is_active(&self) -> bool {
        matches!(self.status(), RunStatus::Running | RunStatus::WaitingConfirmation)
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub use_real_planner: bool,
    pub use_real_browser: bool,
    pub use_real_asr: bool,
    pub use_real_tts: bool,
    pub headless: bool,
    pub adblock_enabled: bool,
    pub default_proxy: Option<String>,
    pub permission_level: PermissionLevel,
    pub static_dir: Option<String>,
    pub worker_dir: String,
}

impl Config {
    pub fn from_env() -> Self {
        let flag = |k: &str| {
            std::env::var(k)
                .map(|v| matches!(v.trim().to_lowercase().as_str(), "1" | "true" | "yes"))
                .unwrap_or(false)
        };
        Self {
            port: std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(4173),
            use_real_planner: flag("USE_REAL_PLANNER"),
            use_real_browser: flag("USE_REAL_BROWSER"),
            use_real_asr: flag("USE_REAL_ASR"),
            use_real_tts: flag("USE_REAL_TTS"),
            headless: std::env::var("HEADLESS")
                .map(|v| v != "false" && v != "0")
                .unwrap_or(true),
            adblock_enabled: std::env::var("ADBLOCK_ENABLED")
                .map(|v| v != "false" && v != "0")
                .unwrap_or(true),
            default_proxy: std::env::var("DEFAULT_PROXY").ok().filter(|v| !v.trim().is_empty()),
            permission_level: match std::env::var("PERMISSION_LEVEL").ok().as_deref() {
                Some("read_only") | Some("1") => PermissionLevel::ReadOnly,
                Some("restricted") | Some("3") => PermissionLevel::Restricted,
                _ => PermissionLevel::Interactive,
            },
            static_dir: std::env::var("STATIC_DIR").ok().or_else(|| {
                let default = "web/dist";
                std::path::Path::new(default).exists().then(|| default.to_string())
            }),
            worker_dir: std::env::var("PLAYWRIGHT_WORKER_DIR")
                .unwrap_or_else(|_| "workers/playwright-worker".into()),
        }
    }
}

pub struct AppState {
    pub config: Config,
    pub events_tx: broadcast::Sender<TraceEvent>,
    pub session: Mutex<SessionState>,
    pub planner: Arc<dyn Planner>,
    pub browser: Arc<dyn BrowserRuntime>,
    pub asr: Arc<dyn AsrAdapter>,
    pub tts: Arc<dyn TtsAdapter>,
    pub policy: PolicyEngine,
}

impl AppState {
    /// Broadcast an event and append it to the session's replayable history.
    pub fn emit(&self, event: TraceEvent) {
        {
            let mut s = self.session.lock().unwrap();
            s.history.push(event.clone());
            let len = s.history.len();
            if len > EVENT_HISTORY_CAP {
                s.history.drain(0..len - EVENT_HISTORY_CAP);
            }
        }
        let _ = self.events_tx.send(event);
    }

    /// Snapshot for `GET /api/state` (also the reconnect recovery path).
    pub fn snapshot(&self) -> serde_json::Value {
        let s = self.session.lock().unwrap();
        serde_json::json!({
            "session_id": s.session_id,
            "status": s.status(),
            "transcript": s.transcript,
            "graph": s.graph,
            "last_success": s.last_success,
            "awaiting_confirmation": s.confirm_tx.is_some(),
            "recent_events": s.history,
            "mode": {
                "planner": if self.config.use_real_planner { "gemini" } else { "mock" },
                "browser": if self.config.use_real_browser { "playwright-sidecar" } else { "mock" },
                "asr": if self.config.use_real_asr { "real" } else { "mock" },
                "tts": if self.config.use_real_tts { "real" } else { "mock" },
                "permission_level": self.config.permission_level,
            }
        })
    }
}
