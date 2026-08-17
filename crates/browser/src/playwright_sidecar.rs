//! PlaywrightSidecarRuntime — the recommended *real* browser runtime.
//!
//! Spawns `workers/playwright-worker` (a small Node process) and speaks a
//! JSON-lines protocol over stdin/stdout:
//!
//! request:  {"id":1,"cmd":"navigate","params":{"url":"https://…"}}
//! response: {"id":1,"ok":true,"page_state":{…},"checks":[…],"duration_ms":812}
//!
//! Only structured commands cross the boundary — the worker refuses anything
//! that is not in its command table, and there is no "evaluate JS" command.

use crate::runtime::BrowserRuntime;
use async_trait::async_trait;
use neuralnav_core::{
    ActionResult, BrowserError, FailureClass, NeuralNavAction, PageState, ScrollDirection,
    VerificationCheck, VerificationResult,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{oneshot, Mutex};
use tokio_util::sync::CancellationToken;

pub struct PlaywrightSidecarRuntime {
    worker_dir: String,
    headless: bool,
    adblock_domains: Vec<String>,
    proxy: Option<String>,
    inner: Mutex<Option<WorkerHandle>>,
    next_id: AtomicU64,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Value>>>>,
}

struct WorkerHandle {
    child: Child,
    stdin: ChildStdin,
}

impl PlaywrightSidecarRuntime {
    pub fn new(
        worker_dir: impl Into<String>,
        headless: bool,
        adblock_domains: Vec<String>,
        proxy: Option<String>,
    ) -> Self {
        Self {
            worker_dir: worker_dir.into(),
            headless,
            adblock_domains,
            proxy,
            inner: Mutex::new(None),
            next_id: AtomicU64::new(1),
            pending: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn ensure_worker(&self) -> Result<(), BrowserError> {
        let mut guard = self.inner.lock().await;
        if guard.is_some() {
            return Ok(());
        }

        let mut cmd = Command::new("node");
        cmd.arg("index.js")
            .current_dir(&self.worker_dir)
            .env("HEADLESS", if self.headless { "1" } else { "0" })
            .env("ADBLOCK_DOMAINS", self.adblock_domains.join(","))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true);
        if let Some(proxy) = &self.proxy {
            cmd.env("PROXY_URL", proxy);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| BrowserError::Unavailable(format!("spawn playwright worker: {e}")))?;
        let stdin = child.stdin.take().ok_or_else(|| {
            BrowserError::Unavailable("worker stdin unavailable".into())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            BrowserError::Unavailable("worker stdout unavailable".into())
        })?;

        // Response pump: route JSON lines back to their pending waiters.
        let pending = self.pending.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(v) = serde_json::from_str::<Value>(&line) {
                    if let Some(id) = v.get("id").and_then(Value::as_u64) {
                        if let Some(tx) = pending.lock().await.remove(&id) {
                            let _ = tx.send(v);
                        }
                    } else {
                        tracing::debug!(target: "playwright", line = %line, "worker log");
                    }
                }
            }
            // Worker exited: fail all pending waiters.
            pending.lock().await.clear();
        });

        *guard = Some(WorkerHandle { child, stdin });
        Ok(())
    }

    async fn request(
        &self,
        cmd: &str,
        params: Value,
        signal: &CancellationToken,
        timeout: Duration,
    ) -> Result<Value, BrowserError> {
        self.ensure_worker().await?;
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);

        let line = serde_json::to_string(&json!({ "id": id, "cmd": cmd, "params": params }))
            .expect("serializable request");
        {
            let mut guard = self.inner.lock().await;
            let handle = guard
                .as_mut()
                .ok_or_else(|| BrowserError::Unavailable("worker not running".into()))?;
            handle
                .stdin
                .write_all(format!("{line}\n").as_bytes())
                .await
                .map_err(|e| BrowserError::Unavailable(format!("worker write: {e}")))?;
            handle
                .stdin
                .flush()
                .await
                .map_err(|e| BrowserError::Unavailable(format!("worker flush: {e}")))?;
        }

        tokio::select! {
            res = rx => res.map_err(|_| BrowserError::Unavailable("worker dropped response".into())),
            _ = signal.cancelled() => {
                self.pending.lock().await.remove(&id);
                Err(BrowserError::Cancelled)
            }
            _ = tokio::time::sleep(timeout) => {
                self.pending.lock().await.remove(&id);
                Err(BrowserError::Action {
                    class: FailureClass::PageTimeout,
                    message: format!("worker command '{cmd}' timed out after {}s", timeout.as_secs()),
                })
            }
        }
    }
}

fn action_to_command(action: &NeuralNavAction) -> Option<(&'static str, Value)> {
    use NeuralNavAction as A;
    Some(match action {
        A::Navigate { url } => ("navigate", json!({ "url": url })),
        A::Click { selector, role, name, text } => (
            "click",
            json!({ "selector": selector, "role": role, "name": name, "text": text }),
        ),
        A::Type { selector, role, name, text } => (
            "type",
            json!({ "selector": selector, "role": role, "name": name, "text": text }),
        ),
        A::Scroll { direction, amount } => (
            "scroll",
            json!({
                "direction": match direction { ScrollDirection::Up => "up", ScrollDirection::Down => "down" },
                "amount": amount,
            }),
        ),
        A::Wait { ms } => ("wait", json!({ "ms": ms })),
        A::Extract { fields } => ("extract", json!({ "fields": fields })),
        A::GoBack => ("go_back", json!({})),
        A::Reload => ("reload", json!({})),
        // Voice-layer actions never reach the worker.
        A::Speak { .. } | A::AskUser { .. } | A::ConfirmSensitiveAction { .. } => return None,
    })
}

fn parse_failure_class(s: &str) -> FailureClass {
    match s {
        "element_not_found" => FailureClass::ElementNotFound,
        "page_timeout" => FailureClass::PageTimeout,
        "popup_blocking_view" => FailureClass::PopupBlockingView,
        "auth_required" => FailureClass::AuthRequired,
        "captcha_required" => FailureClass::CaptchaRequired,
        "network_error" => FailureClass::NetworkError,
        _ => FailureClass::Unknown,
    }
}

fn parse_page_state(v: Option<&Value>) -> Option<PageState> {
    v.and_then(|s| serde_json::from_value(s.clone()).ok())
}

fn parse_checks(v: Option<&Value>) -> Vec<VerificationCheck> {
    v.and_then(|c| serde_json::from_value(c.clone()).ok()).unwrap_or_default()
}

#[async_trait]
impl BrowserRuntime for PlaywrightSidecarRuntime {
    async fn execute(
        &self,
        action: NeuralNavAction,
        signal: CancellationToken,
    ) -> Result<ActionResult, BrowserError> {
        let started = Instant::now();
        let Some((cmd, params)) = action_to_command(&action) else {
            // Voice-layer action: trivially verified no-op at browser level.
            let checks = vec![VerificationCheck::pass("no browser action required", action.kind())];
            return Ok(ActionResult {
                ok: true,
                action,
                verification: VerificationResult::from_checks(checks),
                page_state: None,
                error_class: None,
                duration_ms: 0,
                extracted: None,
            });
        };

        let resp = self
            .request(cmd, params, &signal, Duration::from_secs(30))
            .await?;

        let ok = resp.get("ok").and_then(Value::as_bool).unwrap_or(false);
        let checks = parse_checks(resp.get("checks"));
        let page_state = parse_page_state(resp.get("page_state"));
        let extracted = resp.get("extracted").cloned();

        if !ok {
            let class = resp
                .get("error_class")
                .and_then(Value::as_str)
                .map(parse_failure_class)
                .unwrap_or(FailureClass::Unknown);
            let message = resp
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("worker reported failure")
                .to_string();
            return Err(BrowserError::Action { class, message });
        }

        let verification = VerificationResult::from_checks(if checks.is_empty() {
            vec![VerificationCheck::pass("worker acknowledged", cmd)]
        } else {
            checks
        });
        Ok(ActionResult {
            ok: verification.passed,
            error_class: if verification.passed {
                None
            } else {
                Some(FailureClass::ActionVerificationFailed)
            },
            action,
            verification,
            page_state,
            duration_ms: started.elapsed().as_millis() as u64,
            extracted,
        })
    }

    async fn page_state(&self) -> Result<PageState, BrowserError> {
        let resp = self
            .request(
                "get_page_state",
                json!({}),
                &CancellationToken::new(),
                Duration::from_secs(10),
            )
            .await?;
        parse_page_state(resp.get("page_state"))
            .ok_or_else(|| BrowserError::Unavailable("worker returned no page state".into()))
    }

    async fn stop(&self) -> Result<(), BrowserError> {
        let mut guard = self.inner.lock().await;
        if let Some(mut handle) = guard.take() {
            // Best-effort polite stop, then kill.
            let _ = handle.stdin.write_all(b"{\"id\":0,\"cmd\":\"stop\",\"params\":{}}\n").await;
            let _ = handle.stdin.flush().await;
            let _ = tokio::time::timeout(Duration::from_millis(500), handle.child.wait()).await;
            let _ = handle.child.kill().await;
        }
        Ok(())
    }
}
