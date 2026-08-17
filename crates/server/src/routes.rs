//! HTTP API surface.

use crate::events::sse_events;
use crate::session::{run_command, CommandRequest};
use crate::state::{AppState, RunStatus, SessionState};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use neuralnav_core::TraceEvent;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

pub fn router(state: Arc<AppState>) -> Router {
    let api = Router::new()
        .route("/state", get(get_state))
        .route("/session/start", post(session_start))
        .route("/command", post(command))
        .route("/stop", post(stop))
        .route("/confirm", post(confirm))
        .route("/events", get(sse_events))
        .with_state(state.clone());

    let mut app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .nest("/api", api);

    if let Some(dir) = &state.config.static_dir {
        let serve = tower_http::services::ServeDir::new(dir)
            .fallback(tower_http::services::ServeFile::new(format!("{dir}/index.html")));
        app = app.fallback_service(serve);
    }
    app
}

async fn get_state(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(state.snapshot())
}

async fn session_start(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // Cancel any active run, then reset to a fresh session.
    let (prev_cancel, session_id) = {
        let mut s = state.session.lock().unwrap();
        let prev = s.cancel.take();
        let id = uuid::Uuid::new_v4().to_string();
        *s = SessionState { session_id: Some(id.clone()), ..Default::default() };
        (prev, id)
    };
    if let Some(tok) = prev_cancel {
        tok.cancel();
        state.tts.stop();
    }
    state.emit(TraceEvent::SessionStarted {
        session_id: session_id.clone(),
        timestamp: now_ts(),
    });
    Json(json!({ "session_id": session_id }))
}

#[derive(Debug, Deserialize)]
struct CommandBody {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    audio: bool,
    #[serde(default)]
    demo: bool,
}

async fn command(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CommandBody>,
) -> impl IntoResponse {
    let token = CancellationToken::new();
    let session_id = {
        let mut s = state.session.lock().unwrap();
        if s.is_active() {
            return (
                StatusCode::CONFLICT,
                Json(json!({ "error": "a run is already active; POST /api/stop first" })),
            );
        }
        // Fresh run state (new session id if none exists yet).
        let id = uuid::Uuid::new_v4().to_string();
        *s = SessionState {
            session_id: Some(id.clone()),
            status: Some(RunStatus::Running),
            cancel: Some(token.clone()),
            ..Default::default()
        };
        id
    };
    state.emit(TraceEvent::SessionStarted {
        session_id: session_id.clone(),
        timestamp: now_ts(),
    });

    let req = CommandRequest { text: body.text, audio: body.audio, demo: body.demo };
    tokio::spawn(run_command(state.clone(), req, token));

    (StatusCode::ACCEPTED, Json(json!({ "session_id": session_id, "status": "running" })))
}

#[derive(Debug, Deserialize)]
struct StopBody {
    #[serde(default)]
    reason: Option<String>,
}

async fn stop(
    State(state): State<Arc<AppState>>,
    body: Option<Json<StopBody>>,
) -> impl IntoResponse {
    let reason = body.and_then(|b| b.0.reason);
    let cancelled = {
        let mut s = state.session.lock().unwrap();
        // Resolve any parked confirmation as denied so the run unblocks.
        s.confirm_tx.take();
        match s.cancel.take() {
            Some(tok) => {
                tok.cancel();
                true
            }
            None => false,
        }
    };
    state.tts.stop();
    let _ = state.browser.stop().await;
    state.emit(TraceEvent::UserStopped { reason });
    Json(json!({ "stopped": cancelled }))
}

#[derive(Debug, Deserialize)]
struct ConfirmBody {
    approved: bool,
}

async fn confirm(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ConfirmBody>,
) -> impl IntoResponse {
    let tx = {
        let mut s = state.session.lock().unwrap();
        s.confirm_tx.take()
    };
    match tx {
        Some(tx) => {
            let _ = tx.send(body.approved);
            (StatusCode::OK, Json(json!({ "resolved": true, "approved": body.approved })))
        }
        None => (
            StatusCode::CONFLICT,
            Json(json!({ "error": "no confirmation pending" })),
        ),
    }
}

fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
