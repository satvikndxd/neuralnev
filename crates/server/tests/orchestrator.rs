//! End-to-end orchestrator tests over fully mocked adapters — no network,
//! no keys, no browser.

use neuralnav_core::{PermissionLevel, TraceEvent};
use neuralnav_server::session::{run_command, CommandRequest};
use neuralnav_server::state::{Config, RunStatus};
use neuralnav_server::mock_app_state;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

fn test_config() -> Config {
    Config {
        port: 0,
        use_real_planner: false,
        use_real_browser: false,
        use_real_asr: false,
        use_real_tts: false,
        headless: true,
        adblock_enabled: true,
        default_proxy: None,
        permission_level: PermissionLevel::Interactive,
        static_dir: None,
        worker_dir: String::new(),
    }
}

async fn collect_until_complete(
    rx: &mut tokio::sync::broadcast::Receiver<TraceEvent>,
    timeout: Duration,
) -> Vec<TraceEvent> {
    let mut events = Vec::new();
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let ev = tokio::time::timeout_at(deadline, rx.recv())
            .await
            .expect("run did not complete in time")
            .expect("event channel closed");
        let done = matches!(ev, TraceEvent::SessionCompleted { .. });
        events.push(ev);
        if done {
            return events;
        }
    }
}

#[tokio::test]
async fn demo_runs_end_to_end_under_eight_seconds() {
    let state = mock_app_state(test_config());
    let mut rx = state.events_tx.subscribe();
    let token = CancellationToken::new();
    {
        let mut s = state.session.lock().unwrap();
        s.status = Some(RunStatus::Running);
        s.cancel = Some(token.clone());
    }

    let started = std::time::Instant::now();
    tokio::spawn(run_command(
        state.clone(),
        CommandRequest { text: None, audio: false, demo: true },
        token,
    ));

    let events = collect_until_complete(&mut rx, Duration::from_secs(10)).await;
    assert!(
        started.elapsed() < Duration::from_secs(8),
        "demo took {:?}, budget is 8s",
        started.elapsed()
    );

    // Plan created with the canonical 5 nodes.
    let plan = events
        .iter()
        .find_map(|e| match e {
            TraceEvent::PlanCreated { graph } => Some(graph.clone()),
            _ => None,
        })
        .expect("PlanCreated emitted");
    let ids: Vec<_> = plan.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, ["navigate", "search", "filter", "rank", "choose"]);

    // Every node dispatched and verified.
    let verified: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            TraceEvent::ActionVerified { node_id, result } if result.ok => Some(node_id.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(verified, ["navigate", "search", "filter", "rank", "choose"]);

    // Final transcript + spoken feedback + success.
    assert!(events.iter().any(|e| matches!(
        e,
        TraceEvent::VoiceFinalTranscript { text } if text.contains("mechanical keyboard")
    )));
    assert!(events.iter().any(|e| matches!(e, TraceEvent::TtsSpoken { .. })));
    assert!(matches!(
        events.last().unwrap(),
        TraceEvent::SessionCompleted { success: true }
    ));

    // Session state settled.
    let s = state.session.lock().unwrap();
    assert_eq!(s.status(), RunStatus::Completed);
    assert_eq!(s.last_success, Some(true));
}

#[tokio::test]
async fn stop_cancels_the_run_immediately() {
    let state = mock_app_state(test_config());
    let mut rx = state.events_tx.subscribe();
    let token = CancellationToken::new();
    {
        let mut s = state.session.lock().unwrap();
        s.status = Some(RunStatus::Running);
        s.cancel = Some(token.clone());
    }

    tokio::spawn(run_command(
        state.clone(),
        CommandRequest { text: None, audio: false, demo: true },
        token.clone(),
    ));

    // Let it get into the middle of the run, then barge in.
    tokio::time::sleep(Duration::from_millis(900)).await;
    let stop_at = std::time::Instant::now();
    token.cancel();
    state.emit(TraceEvent::UserStopped { reason: Some("test".into()) });

    let events = collect_until_complete(&mut rx, Duration::from_secs(3)).await;
    assert!(
        stop_at.elapsed() < Duration::from_millis(500),
        "cancellation should settle fast, took {:?}",
        stop_at.elapsed()
    );
    assert!(matches!(
        events.last().unwrap(),
        TraceEvent::SessionCompleted { success: false }
    ));
    let s = state.session.lock().unwrap();
    assert_eq!(s.status(), RunStatus::Stopped);
    assert_eq!(s.last_success, Some(false));
}

#[tokio::test]
async fn ambiguous_command_requests_clarification_and_resumes_on_confirm() {
    let state = mock_app_state(test_config());
    let mut rx = state.events_tx.subscribe();
    let token = CancellationToken::new();
    {
        let mut s = state.session.lock().unwrap();
        s.status = Some(RunStatus::Running);
        s.cancel = Some(token.clone());
    }

    tokio::spawn(run_command(
        state.clone(),
        CommandRequest { text: Some("open the second one".into()), audio: false, demo: false },
        token,
    ));

    // Wait for the confirmation request, then approve it (as /api/confirm would).
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let ev = tokio::time::timeout_at(deadline, rx.recv())
            .await
            .expect("no confirmation requested")
            .unwrap();
        if matches!(ev, TraceEvent::UserConfirmationRequested { .. }) {
            break;
        }
    }
    let tx = state.session.lock().unwrap().confirm_tx.take().expect("confirm pending");
    tx.send(true).unwrap();

    let events = collect_until_complete(&mut rx, Duration::from_secs(5)).await;
    assert!(events
        .iter()
        .any(|e| matches!(e, TraceEvent::UserConfirmationResolved { approved: true })));
    assert!(matches!(
        events.last().unwrap(),
        TraceEvent::SessionCompleted { success: true }
    ));
}
