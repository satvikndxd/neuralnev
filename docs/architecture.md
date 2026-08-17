# Architecture

## Design philosophy

Never `user says → LLM directly controls browser`. Always:

```
LLM proposes structured action → validator approves → policy gates
→ executor runs → verifier confirms → recovery routes failures
```

Deterministic where possible, LLM-assisted where needed.

## Layers

```
┌────────────────────────── web/ (Vite + TS) ──────────────────────────┐
│  console view ← pure reducer ← reconnecting SSE client ← /api/state │
└───────────────────────────────▲──────────────────────────────────────┘
                                │ SSE (TraceEvent JSON)
┌───────────────────────── crates/server ─────────────────────────────┐
│ routes.rs   /api/command /stop /confirm /state /events              │
│ session.rs  orchestrator: transcript → intent → plan → per node:    │
│             policy gate → dispatch → verify → recover               │
│ state.rs    AppState { broadcast bus, session, adapters, policy }   │
└───────┬───────────────┬────────────────┬──────────────┬─────────────┘
        ▼               ▼                ▼              ▼
  crates/planner   crates/browser   crates/voice   crates/guardrails
  Planner trait    BrowserRuntime   AsrAdapter     PolicyEngine
  Mock / Gemini    Mock / Sidecar   TtsAdapter     3 permission levels
                   (+ cdp stub)     BargeIn
        ▲               ▲                ▲              ▲
        └───────────────┴── crates/core ─┴──────────────┘
             actions · task graph · events · failure classes · schema
```

## The run lifecycle

1. `POST /api/command` — creates a fresh session + `CancellationToken`,
   emits `SessionStarted`, spawns the orchestrator, returns `202`.
2. **Transcript** — demo text, provided text, or `AsrAdapter`; partials are
   streamed word-by-word (`VoicePartialTranscript` → `VoiceFinalTranscript`).
3. **Intent** — cheap heuristic classification → `IntentParsed`.
4. **Plan** — `Planner::plan` returns a `TaskGraph`; it is re-validated
   (unique ids, resolvable deps, acyclic, non-empty success checks) →
   `PlanCreated`.
5. **Execute** — nodes run in topological order. Per node:
   - skip if a dependency didn't succeed;
   - `PolicyEngine::evaluate` → Allowed / Blocked (`ActionFailed:
     policy_blocked`) / ConfirmationRequired (park on a oneshot until
     `/api/confirm` or stop);
   - voice actions (`speak`, `ask_user`) handled by the orchestrator;
   - browser actions: `ActionDispatched` → `BrowserRuntime::execute` →
     `ActionVerified` (checks attached) or `ActionFailed` + classified
     recovery (`RecoveryAttempted`) with bounded retries;
   - concise TTS line fired concurrently so speech overlaps execution.
6. **Finish** — `SessionCompleted { success }`; on cancellation the session
   ends `stopped` with pending nodes skipped.

## Cancellation

One `CancellationToken` per run, threaded into every sleep, TTS utterance,
browser action and confirmation wait (`tokio::select!`). `POST /api/stop`
cancels the token, hard-stops TTS, resolves any pending confirmation as
denied, and asks the browser runtime to stop. Measured: the run settles in
well under 500 ms (asserted by `stop_cancels_the_run_immediately`).

## Events & reconnection

`tokio::sync::broadcast` fans `TraceEvent`s out to every SSE subscriber;
each event also lands in a 256-entry session history. Clients that connect
late (or reconnect after a drop or a `lagged` frame) refetch `/api/state`
and replay `recent_events` through the same reducer as the live stream —
one code path for both.

## Why a Node sidecar instead of pure-Rust CDP

Playwright's role/name locators and auto-waiting are exactly what the
self-healing selector ladder needs; the pure-Rust CDP crates don't offer
them reliably. The boundary is a JSON-lines protocol of structured commands
(no `evaluate`), so the sidecar has the same "no arbitrary code" guarantee
as the rest of the system. The `cdp` feature flag reserves the slot for a
future pure-Rust runtime.
