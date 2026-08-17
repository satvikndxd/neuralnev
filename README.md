# NeuralNav — Voice-First Autonomous Browser Agent

NeuralNav interprets a spoken (or typed) command, decomposes it into a
**validated task graph**, executes **structured browser actions** through a
trait-based `BrowserRuntime`, **verifies every step**, recovers from
classified failures, and streams the whole trace live to a Bauhaus-styled web
console — with concise spoken feedback and instant barge-in.

Rust-first architecture: Axum + Tokio backend, Vite + TypeScript frontend,
SSE event streaming, mock-first adapters (the full demo runs with **zero API
keys, no microphone, and no real browser**).

```text
Voice/Text ─▶ ASR ─▶ Intent ─▶ Planner ─▶ Policy ─▶ BrowserRuntime ─▶ Verifier ─▶ TTS
  (mock)     trait   heuristic  trait      engine       trait           checks    trait
                     + Gemini   mock/LLM   3 levels   mock / Playwright  every     mock
                                                        sidecar         step
                          │                                │
                          └────────── TraceEvent bus ──────┘
                                        │ SSE
                                  Web console (Vite/TS)
```

## Quickstart

```sh
# 1. Backend (mock mode — no keys needed)
cargo run -p neuralnav-server            # → http://localhost:4173

# 2. Frontend
cd web && pnpm install && pnpm dev       # → http://localhost:5173 (proxies /api)

# or serve the built frontend from the Rust binary:
cd web && pnpm install && pnpm build && cd .. && cargo run -p neuralnav-server
# → everything on http://localhost:4173
```

Open the console, press **▶ Run demo**, and watch:
transcript streams in → intent parsed → 5-node task graph → each action
dispatched as strict JSON → verifier checks tick green → TTS lines land →
**■ Barge-in / stop** halts everything mid-flight.

The demo command:
> "Open Amazon and find a mechanical keyboard under 5,000 rupees with good reviews."

### Scripts

| What | Command |
| --- | --- |
| Run the server | `cargo run -p neuralnav-server` |
| Run the web frontend | `cd web && pnpm dev` |
| Run the demo (headless) | `curl -X POST localhost:4173/api/command -H 'Content-Type: application/json' -d '{"demo":true}'` then `curl -N localhost:4173/api/events` |
| Rust tests | `cargo test` |
| Frontend tests | `cd web && pnpm test` |
| Planner evals | `cargo run -p neuralnav-evals --bin run-evals` |

## Environment variables

All optional — defaults give full mock mode. See `.env.example`.

| Var | Default | Meaning |
| --- | --- | --- |
| `PORT` | `4173` | HTTP port |
| `RUST_LOG` | `info` | tracing filter |
| `USE_REAL_PLANNER` | `false` | `true` → GeminiPlanner (falls back to mock on any failure) |
| `GEMINI_API_KEY` / `GEMINI_MODEL` | — / `gemini-2.0-flash` | Gemini credentials |
| `USE_REAL_BROWSER` | `false` | `true` → Playwright sidecar (`workers/playwright-worker`, needs `npm install` there) |
| `HEADLESS` | `true` | show/hide the sidecar's Chromium |
| `ADBLOCK_ENABLED` | `true` | request-interception filter list |
| `DEFAULT_PROXY` | — | `name=url,name=url` proxy pool for the sidecar |
| `PERMISSION_LEVEL` | `interactive` | `read_only` \| `interactive` \| `restricted` |
| `USE_REAL_ASR` / `USE_REAL_TTS` | `false` | reserved — no vendor bundled; logs a warning and uses mocks |

## Mock vs real mode

**Mock (default).** Deterministic, no keys, no network: `MockPlanner`
(keyword task-graphs + clarification for ambiguous commands),
`MockBrowserRuntime` (simulated Amazon flow with realistic latencies and per-
action verification), `MockAsr`/`MockTts` (cancellable). The full demo
finishes in ~3.5 s.

**Real.** `USE_REAL_PLANNER=true` sends transcript + page state + policy
constraints to Gemini demanding strict JSON, validates through the same serde
schema as everything else, retries once, then falls back to mock.
`USE_REAL_BROWSER=true` spawns the Node Playwright worker and speaks a
JSON-lines protocol of **structured commands only** — there is no
"evaluate JS" command, by design.

## API

| Endpoint | Purpose |
| --- | --- |
| `GET /health` | liveness |
| `GET /api/state` | session snapshot (status, transcript, graph, recent events, adapter modes) |
| `POST /api/session/start` | reset to a fresh session |
| `POST /api/command` `{text?, audio?, demo?}` | start a run (`409` if one is active) |
| `POST /api/stop` | cancel the run, stop TTS, emit `UserStopped` |
| `POST /api/confirm` `{approved}` | resolve a pending confirmation/clarification |
| `GET /api/events` | SSE stream of `TraceEvent` JSON |

## Crates

| Crate | Contents |
| --- | --- |
| `neuralnav-core` | `NeuralNavAction` (closed enum — no code execution), `TaskGraph` + DAG validation, `PageState`, `ActionResult`/`VerificationCheck`, `FailureClass`, `TraceEvent`, planner-output schema validation |
| `neuralnav-planner` | `Planner` trait, intent heuristics, `MockPlanner`, `GeminiPlanner` (strict-JSON prompt, validate → retry → fallback) |
| `neuralnav-browser` | `BrowserRuntime` trait, `MockBrowserRuntime`, `PlaywrightSidecarRuntime` (JSON-lines stdio), verifier helpers, failure→recovery routing, adblock engine, proxy manager, `cdp` feature stub |
| `neuralnav-voice` | `AsrAdapter`/`TtsAdapter` traits, cancellable mocks, `BargeInController` |
| `neuralnav-guardrails` | policy engine: permission levels → Allowed / Blocked / ConfirmationRequired; CAPTCHA never automated |
| `neuralnav-server` | Axum routes, SSE broadcast, session orchestrator (plan → gate → execute → verify → recover → speak), cancellation everywhere |
| `neuralnav-evals` | labelled command dataset, intent/clarification/graph-validity metrics, `run-evals` binary |

## Frontend (`web/`)

Vite + strict TypeScript, no framework. A pure reducer
(`src/state/store.ts`, unit-tested) folds `TraceEvent`s into console state;
views render it. Reconnecting SSE client heals gaps by replaying the
`/api/state` snapshot through the same reducer. Strict Bauhaus visual
language: cream paper, ink 3 px rules, red/yellow/blue, circle/triangle/
square, Jost type, hard offset shadows — no gradients, no glassmorphism.
Responsive to phone width, `prefers-reduced-motion` respected, live regions
on the log panels, all controls keyboard-reachable.

## Safety model

- **Structured actions only** — the planner's output is deserialized into a
  closed enum; unknown action types fail validation before reaching any
  browser.
- **Verify every action** — no "dispatched = succeeded"; every node returns
  checks, failures are classified and routed to a recovery strategy.
- **Permission levels** — read-only blocks mutation; interactive/restricted
  require human confirmation for payment/checkout/message/deletion-shaped
  actions; CAPTCHAs are never automated.
- **Cancellation everywhere** — one `CancellationToken` per run; stop/barge-in
  halts TTS mid-word and aborts the in-flight browser action.

## Limitations

- One active session at a time (`409` otherwise).
- Real ASR/TTS adapters are trait stubs — audio capture belongs client-side.
- The pure-Rust CDP runtime is a reserved feature flag, not an implementation.
- The sidecar's `extract` is generic; site-specific extraction is planner-side.
- See `ASSUMPTIONS.md` for the full list.

## Roadmap

Session multiplexing → Web Speech API capture in the console → carry
clarification answers into a re-plan loop → vision-fallback grounding in the
sidecar → full EasyList ingestion → CDP runtime once role/name locators are
viable in pure Rust.

## Docs

`docs/architecture.md` · `docs/actions.md` · `docs/guardrails.md` ·
`docs/evals.md` · `workers/playwright-worker/README.md` · `ASSUMPTIONS.md`
