//! The orchestrator: transcript → intent → plan → policy-gated execution →
//! verification → recovery → spoken feedback. Fully cancellable at every
//! await point via the run's CancellationToken.

use crate::state::{AppState, RunStatus};
use neuralnav_browser::recovery::{strategy_for, strategy_label, RecoveryStrategy};
use neuralnav_core::{
    BrowserError, FailureClass, NeuralNavAction, PlannerInput, TaskStatus, TraceEvent,
};
use neuralnav_guardrails::PolicyDecision;
use neuralnav_voice::AudioInput;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

pub const DEMO_COMMAND: &str =
    "Open Amazon and find a mechanical keyboard under 5,000 rupees with good reviews.";

pub struct CommandRequest {
    pub text: Option<String>,
    pub audio: bool,
    pub demo: bool,
}

/// Entry point, spawned as a background task by `POST /api/command`.
pub async fn run_command(state: Arc<AppState>, req: CommandRequest, token: CancellationToken) {
    let success = match drive(state.clone(), req, token.clone()).await {
        Ok(success) => success,
        Err(RunAbort::Cancelled) => {
            finish(&state, false, RunStatus::Stopped);
            return;
        }
        Err(RunAbort::Failed(reason)) => {
            tracing::warn!(%reason, "run aborted");
            speak_now(&state, "I couldn't complete that.", &token).await;
            finish(&state, false, RunStatus::Completed);
            return;
        }
    };
    finish(&state, success, RunStatus::Completed);
}

fn finish(state: &AppState, success: bool, status: RunStatus) {
    {
        let mut s = state.session.lock().unwrap();
        s.status = Some(status);
        s.last_success = Some(success);
        s.cancel = None;
        s.confirm_tx = None;
        // Any node still pending/running is marked appropriately.
        if let Some(graph) = &mut s.graph {
            for n in &mut graph.nodes {
                if matches!(n.status, TaskStatus::Pending | TaskStatus::Running | TaskStatus::WaitingUser) {
                    n.status = if status == RunStatus::Stopped {
                        TaskStatus::Skipped
                    } else {
                        n.status
                    };
                }
            }
        }
    }
    state.emit(TraceEvent::SessionCompleted { success });
}

enum RunAbort {
    Cancelled,
    Failed(String),
}

impl From<BrowserError> for RunAbort {
    fn from(e: BrowserError) -> Self {
        match e {
            BrowserError::Cancelled => RunAbort::Cancelled,
            other => RunAbort::Failed(other.to_string()),
        }
    }
}

async fn drive(
    state: Arc<AppState>,
    req: CommandRequest,
    token: CancellationToken,
) -> Result<bool, RunAbort> {
    // ── 1. Transcript ────────────────────────────────────────────────
    let transcript = if req.demo {
        DEMO_COMMAND.to_string()
    } else if let Some(text) = req.text.filter(|t| !t.trim().is_empty()) {
        text
    } else if req.audio {
        state
            .asr
            .transcribe(AudioInput { bytes: vec![], hint: None })
            .await
            .map_err(|e| RunAbort::Failed(e.to_string()))?
    } else {
        return Err(RunAbort::Failed("no command provided".into()));
    };

    // Simulated streaming partials (word by word) — cancellable.
    let words: Vec<&str> = transcript.split_whitespace().collect();
    let mut partial = String::new();
    for w in &words {
        cancellable_sleep(Duration::from_millis(55), &token).await?;
        if !partial.is_empty() {
            partial.push(' ');
        }
        partial.push_str(w);
        state.emit(TraceEvent::VoicePartialTranscript { text: partial.clone() });
    }
    state.emit(TraceEvent::VoiceFinalTranscript { text: transcript.clone() });
    {
        let mut s = state.session.lock().unwrap();
        s.transcript = Some(transcript.clone());
    }

    // ── 2. Intent ────────────────────────────────────────────────────
    let intent = state.planner.classify_intent(&transcript);
    state.emit(TraceEvent::IntentParsed {
        intent: intent.intent.clone(),
        confidence: intent.confidence,
    });

    // ── 3. Plan ──────────────────────────────────────────────────────
    let page_state = state.browser.page_state().await.ok();
    let graph = state
        .planner
        .plan(PlannerInput {
            transcript: transcript.clone(),
            page_state,
            permission_level: state.config.permission_level,
            prior_actions: vec![],
        })
        .await
        .map_err(|e| RunAbort::Failed(e.to_string()))?;
    graph.validate().map_err(|e| RunAbort::Failed(e.to_string()))?;
    let order = graph.topological_order().map_err(|e| RunAbort::Failed(e.to_string()))?;
    {
        let mut s = state.session.lock().unwrap();
        s.graph = Some(graph.clone());
    }
    state.emit(TraceEvent::PlanCreated { graph: graph.clone() });

    // ── 4. Execute nodes in dependency order ─────────────────────────
    let mut all_ok = true;
    for node_id in order {
        if token.is_cancelled() {
            return Err(RunAbort::Cancelled);
        }
        let (action, tts_line, deps) = {
            let s = state.session.lock().unwrap();
            let g = s.graph.as_ref().unwrap();
            let node = g.node(&node_id).unwrap();
            let tts_line = g
                .metadata
                .as_ref()
                .and_then(|m| m.get("tts"))
                .and_then(|t| t.get(&node_id))
                .and_then(|v| v.as_str())
                .map(String::from);
            (node.action.clone(), tts_line, node.depends_on.clone())
        };

        // Skip nodes whose dependencies did not succeed.
        let deps_ok = {
            let s = state.session.lock().unwrap();
            let g = s.graph.as_ref().unwrap();
            deps.iter().all(|d| {
                g.node(d).map(|n| n.status == TaskStatus::Success).unwrap_or(false)
            })
        };
        if !deps_ok {
            set_node_status(&state, &node_id, TaskStatus::Skipped, None);
            all_ok = false;
            continue;
        }

        // ── 4a. Policy gate ──────────────────────────────────────────
        let decision = state.policy.evaluate(&action, state.config.permission_level);
        match &decision {
            PolicyDecision::Allowed => {
                state.emit(TraceEvent::PolicyDecision {
                    node_id: node_id.clone(),
                    decision: "allowed".into(),
                    reason: None,
                });
            }
            PolicyDecision::Blocked { reason } => {
                state.emit(TraceEvent::PolicyDecision {
                    node_id: node_id.clone(),
                    decision: "blocked".into(),
                    reason: Some(reason.clone()),
                });
                state.emit(TraceEvent::ActionFailed {
                    node_id: node_id.clone(),
                    error_class: FailureClass::PolicyBlocked,
                    detail: Some(reason.clone()),
                });
                set_node_status(&state, &node_id, TaskStatus::Failed, Some(reason.clone()));
                all_ok = false;
                continue;
            }
            PolicyDecision::ConfirmationRequired { description } => {
                state.emit(TraceEvent::PolicyDecision {
                    node_id: node_id.clone(),
                    decision: "confirmation_required".into(),
                    reason: Some(description.clone()),
                });
                let approved =
                    await_confirmation(&state, &node_id, description.clone(), &token).await?;
                if !approved {
                    set_node_status(&state, &node_id, TaskStatus::Skipped, None);
                    speak_now(&state, "Okay, skipping that.", &token).await;
                    all_ok = false;
                    continue;
                }
            }
        }

        // ── 4b. Voice-layer actions handled by the orchestrator ──────
        match &action {
            NeuralNavAction::Speak { message } => {
                set_node_status(&state, &node_id, TaskStatus::Running, None);
                speak_now(&state, message, &token).await;
                set_node_status(&state, &node_id, TaskStatus::Success, None);
                continue;
            }
            NeuralNavAction::AskUser { question, .. } => {
                set_node_status(&state, &node_id, TaskStatus::WaitingUser, None);
                speak_now(&state, question, &token).await;
                let answered =
                    await_confirmation(&state, &node_id, question.clone(), &token).await?;
                set_node_status(
                    &state,
                    &node_id,
                    if answered { TaskStatus::Success } else { TaskStatus::Skipped },
                    None,
                );
                if !answered {
                    all_ok = false;
                }
                continue;
            }
            _ => {}
        }

        // ── 4c. Dispatch to the browser, verify, recover ─────────────
        if let Some(line) = &tts_line {
            speak_soon(&state, line.clone(), &token);
        }
        set_node_status(&state, &node_id, TaskStatus::Running, None);
        state.emit(TraceEvent::ActionDispatched {
            node_id: node_id.clone(),
            action: action.clone(),
        });

        let node_ok = execute_with_recovery(&state, &node_id, &action, &token).await?;
        if !node_ok {
            all_ok = false;
        }
    }

    Ok(all_ok)
}

/// Execute one action with the classify→recover→retry ladder.
async fn execute_with_recovery(
    state: &Arc<AppState>,
    node_id: &str,
    action: &NeuralNavAction,
    token: &CancellationToken,
) -> Result<bool, RunAbort> {
    let mut attempts: u32 = 0;
    loop {
        attempts += 1;
        bump_attempts(state, node_id);

        let outcome = state.browser.execute(action.clone(), token.clone()).await;
        let class = match outcome {
            Ok(result) if result.ok => {
                set_node_status(state, node_id, TaskStatus::Success, None);
                state.emit(TraceEvent::ActionVerified {
                    node_id: node_id.to_string(),
                    result,
                });
                return Ok(true);
            }
            Ok(result) => {
                // Executed but verification failed.
                let class = result
                    .error_class
                    .unwrap_or(FailureClass::ActionVerificationFailed);
                state.emit(TraceEvent::ActionVerified {
                    node_id: node_id.to_string(),
                    result,
                });
                class
            }
            Err(BrowserError::Cancelled) => return Err(RunAbort::Cancelled),
            Err(e) => e.class(),
        };

        state.emit(TraceEvent::ActionFailed {
            node_id: node_id.to_string(),
            error_class: class,
            detail: None,
        });

        let strategy = strategy_for(class, attempts);
        state.emit(TraceEvent::RecoveryAttempted {
            node_id: node_id.to_string(),
            strategy: strategy_label(&strategy),
        });

        match strategy {
            RecoveryStrategy::Retry { .. }
            | RecoveryStrategy::SelfHealSelector
            | RecoveryStrategy::DismissPopups
            | RecoveryStrategy::NetworkFailover => {
                cancellable_sleep(Duration::from_millis(180), token)
                    .await
                    .map_err(|_| RunAbort::Cancelled)?;
                continue;
            }
            RecoveryStrategy::PauseForHuman { reason } => {
                let approved = await_confirmation(
                    state,
                    node_id,
                    format!("Paused: {reason}. Continue when ready?"),
                    token,
                )
                .await?;
                if approved {
                    continue;
                }
                set_node_status(state, node_id, TaskStatus::Failed, Some(reason.to_string()));
                return Ok(false);
            }
            RecoveryStrategy::AskClarification => {
                speak_now(state, "That was ambiguous — could you rephrase?", token).await;
                set_node_status(state, node_id, TaskStatus::Failed, Some("ambiguous".into()));
                return Ok(false);
            }
            RecoveryStrategy::Abort => {
                set_node_status(
                    state,
                    node_id,
                    TaskStatus::Failed,
                    Some(format!("{class:?} after {attempts} attempts")),
                );
                return Ok(false);
            }
        }
    }
}

/// Ask the user to approve/deny; parks the run until `/api/confirm` or stop.
async fn await_confirmation(
    state: &Arc<AppState>,
    _node_id: &str,
    question: String,
    token: &CancellationToken,
) -> Result<bool, RunAbort> {
    let (tx, rx) = oneshot::channel();
    {
        let mut s = state.session.lock().unwrap();
        s.confirm_tx = Some(tx);
        s.status = Some(RunStatus::WaitingConfirmation);
    }
    state.emit(TraceEvent::UserConfirmationRequested { question });

    let approved = tokio::select! {
        res = rx => res.unwrap_or(false),
        _ = token.cancelled() => {
            let mut s = state.session.lock().unwrap();
            s.confirm_tx = None;
            return Err(RunAbort::Cancelled);
        }
    };
    {
        let mut s = state.session.lock().unwrap();
        s.status = Some(RunStatus::Running);
        s.confirm_tx = None;
    }
    state.emit(TraceEvent::UserConfirmationResolved { approved });
    Ok(approved)
}

/// Speak and wait (used for questions / final lines).
async fn speak_now(state: &Arc<AppState>, message: &str, token: &CancellationToken) {
    state.emit(TraceEvent::TtsSpoken { message: message.to_string() });
    let _ = state.tts.speak(message.to_string(), token.clone()).await;
}

/// Fire-and-forget speech so browser work overlaps the utterance.
fn speak_soon(state: &Arc<AppState>, message: String, token: &CancellationToken) {
    state.emit(TraceEvent::TtsSpoken { message: message.clone() });
    let tts = state.tts.clone();
    let token = token.clone();
    tokio::spawn(async move {
        let _ = tts.speak(message, token).await;
    });
}

fn set_node_status(state: &AppState, node_id: &str, status: TaskStatus, error: Option<String>) {
    let mut s = state.session.lock().unwrap();
    if let Some(graph) = &mut s.graph {
        if let Some(node) = graph.node_mut(node_id) {
            node.status = status;
            node.last_error = error;
        }
    }
}

fn bump_attempts(state: &AppState, node_id: &str) {
    let mut s = state.session.lock().unwrap();
    if let Some(graph) = &mut s.graph {
        if let Some(node) = graph.node_mut(node_id) {
            node.attempts += 1;
        }
    }
}

async fn cancellable_sleep(d: Duration, token: &CancellationToken) -> Result<(), RunAbort> {
    tokio::select! {
        _ = tokio::time::sleep(d) => Ok(()),
        _ = token.cancelled() => Err(RunAbort::Cancelled),
    }
}
