# ASSUMPTIONS

Every deliberate simplification or ambiguous-spec decision, in one place.

## Architecture

1. **Single active session.** The server runs one session at a time
   (`POST /api/command` returns `409` while a run is active). Multi-session
   would add a session-id routing layer to every endpoint and the SSE stream
   without changing the architecture; it was cut for simplicity.
2. **CDP runtime is a documented stub.** `chromiumoxide`/`headless_chrome`
   lag Chrome's CDP surface for accessibility-tree queries and role/name
   locators, which the selector ladder depends on. Per the project rule ("do
   not force pure Rust browser automation if it becomes brittle"), the `cdp`
   Cargo feature compiles a stub that returns `Unavailable` and points here.
   Mock and Playwright sidecar are the supported runtimes.
3. **ts-rs type generation was skipped.** The serde types are mirrored by
   hand in `web/src/lib/types.ts` (~120 lines). One small file was judged
   cheaper than a codegen step in the build; the Rust serde tests pin the
   wire format the TS types mirror.

## Voice

4. **Audio never reaches the Rust server.** Real microphone capture/VAD
   belongs in the browser (Web Speech API) or an external ASR service. The
   `AsrAdapter` trait is the integration point; `MockAsr` returns the demo
   utterance. `USE_REAL_ASR/USE_REAL_TTS=true` log a warning and fall back
   to mocks because no external ASR/TTS vendor is bundled (no paid services
   allowed in the demo).
5. **Mock TTS is time-based.** `MockTts` "speaks" by sleeping ~14 ms/char
   (capped at 1.5 s) and is fully cancellable — which is exactly the
   contract a real TTS adapter must satisfy.
6. **Streaming partial transcripts are simulated** word-by-word by the
   orchestrator (55 ms/word) so the UI shows the streaming path even in
   mock mode.

## Planning

7. **MockPlanner is keyword-driven, not a model.** It recognizes the
   canonical demo, `open <known-site>`, ambiguous referring expressions
   ("the second one"), and falls back to a search task-graph. That is enough
   to exercise every orchestrator path (plan, clarify, block, confirm).
8. **GeminiPlanner falls back to MockPlanner** when the key is missing, the
   HTTP call fails, or the output fails schema validation twice (initial +
   one retry with the error appended). The demo therefore never depends on
   Gemini being reachable.
9. **`AskUser` resolves through `/api/confirm`.** A full clarification loop
   would carry the chosen option back into a re-plan. Here "approve" means
   *acknowledged, continue*, "deny" means *skip and end*. The trace events
   (`UserConfirmationRequested/Resolved`) are the same ones a full loop
   would use.

## Browser / verification

10. **Navigation verifies "arrived at destination", not "URL differs".**
    Re-running the demo starts from the previous run's final page;
    re-navigating to a page you're already on is still a success. (Caught by
    the UI smoke test; regression-tested in `navigation_is_replayable`.)
11. **The mock's Amazon flow is a small state machine** (landing → search
    results 1,204 → filtered 312 → extract 5 candidates → product page) with
    realistic latencies; total demo wall-clock is ~3.5 s, well under the 8 s
    budget (enforced by an integration test).
12. **Adblock ships a 32-domain seed list**, not 65k rules. The engine
    parses EasyList-style domain lines (`||domain^`), so a full dump can be
    loaded via `AdblockEngine::from_rules`; embedding 65k rules in the repo
    was judged noise. The worker receives the domain list and aborts
    matching requests.
13. **Proxy health checks are a deterministic stub** (`StubProber`), with
    the probing behind a trait so real HEAD-request probing slots in without
    touching the latency-based selection logic.
14. **Playwright sidecar `extract` is generic** (headings/links/cards), not
    site-specific — the planner only chooses which fields to keep. Fully
    exercising the sidecar requires `npm install` in
    `workers/playwright-worker` (downloads Chromium), so CI-grade tests
    cover the mock runtime and the protocol layer instead.

## Guardrails

15. **Sensitivity is keyword-based** (checkout/payment/delete/send/…) over
    the action's textual surface, evaluated per-action at dispatch time.
    A production system would also classify the *page context* (e.g. a
    checkout URL). CAPTCHA-looking targets are refused at every level.
16. **`stop` resolves pending confirmations as denied** and cancels the
    run token; TTS is hard-stopped. This is the barge-in path; there is no
    separate voice-activity barge-in in the server (that lives client-side).

## Frontend

17. **The metrics section's stat tiles are sourced from the eval harness &
    tests** (10/10 intent, 100% graph validity, <8 s demo, 32 seed rules) —
    not invented percentages. The latency-budget chart is the design-target
    budget, labeled as such.
18. **SSE reconnection heals via snapshot replay**: on every (re)connect the
    client refetches `/api/state` and replays `recent_events` (capped at
    256) through the same reducer the live stream uses.
19. **Frontend tests cover the reducer**, which owns all console state
    transitions (run/stop/confirm), rather than DOM snapshots; the rendered
    UI was verified end-to-end with a scripted Playwright pass against the
    real backend during development.
