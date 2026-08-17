//! NeuralNav server binary. Adapter selection happens here, driven by env
//! vars — everything downstream only sees the traits.

use neuralnav_browser::adblock::AdblockEngine;
use neuralnav_server::{routes, state};
use neuralnav_browser::{BrowserRuntime, MockBrowserRuntime, PlaywrightSidecarRuntime};
use neuralnav_guardrails::PolicyEngine;
use neuralnav_planner::{GeminiPlanner, MockPlanner, Planner};
use neuralnav_voice::{AsrAdapter, MockAsr, MockTts, TtsAdapter};
use state::{AppState, Config, SessionState};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = Config::from_env();

    // ── Planner ──────────────────────────────────────────────────────
    let planner: Arc<dyn Planner> = if config.use_real_planner {
        tracing::info!("planner: Gemini (falls back to mock on failure)");
        Arc::new(GeminiPlanner::new(
            std::env::var("GEMINI_API_KEY").ok(),
            std::env::var("GEMINI_MODEL").ok(),
        ))
    } else {
        tracing::info!("planner: mock");
        Arc::new(MockPlanner)
    };

    // ── Browser runtime ──────────────────────────────────────────────
    let adblock = AdblockEngine::with_default_rules(config.adblock_enabled);
    tracing::info!(rules = adblock.rule_count(), enabled = config.adblock_enabled, "adblock");
    let browser: Arc<dyn BrowserRuntime> = if config.use_real_browser {
        tracing::info!(dir = %config.worker_dir, "browser: playwright sidecar");
        Arc::new(PlaywrightSidecarRuntime::new(
            config.worker_dir.clone(),
            config.headless,
            adblock.domains(),
            config.default_proxy.clone(),
        ))
    } else {
        tracing::info!("browser: mock");
        Arc::new(MockBrowserRuntime::default())
    };

    // ── Voice ────────────────────────────────────────────────────────
    // Real ASR/TTS adapters are not bundled (audio stays in the browser via
    // the Web Speech API); the flags log a warning and use mocks. See
    // ASSUMPTIONS.md.
    if config.use_real_asr {
        tracing::warn!("USE_REAL_ASR=true but no server-side ASR adapter is bundled; using mock");
    }
    if config.use_real_tts {
        tracing::warn!("USE_REAL_TTS=true but no server-side TTS adapter is bundled; using mock");
    }
    let asr: Arc<dyn AsrAdapter> = Arc::new(MockAsr::default());
    let tts: Arc<dyn TtsAdapter> = Arc::new(MockTts::default());

    let (events_tx, _) = broadcast::channel(256);
    let state = Arc::new(AppState {
        config: config.clone(),
        events_tx,
        session: Mutex::new(SessionState::default()),
        planner,
        browser,
        asr,
        tts,
        policy: PolicyEngine::new(),
    });

    let app = routes::router(state).layer(CorsLayer::permissive());

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!(%addr, "NeuralNav server listening");
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    axum::serve(listener, app).await.expect("serve");
}
