# Action contract

The planner can only emit members of `NeuralNavAction` (crates/core). The
enum is the schema: serde deserialization with a closed tag set *is* the
validation, and it happens before an action can reach any runtime.

```jsonc
{"type":"navigate","url":"https://www.amazon.in"}
{"type":"click","role":"link","name":"Under ₹5,000"}          // selector/text optional
{"type":"type","role":"textbox","name":"Search","text":"mechanical keyboard"}
{"type":"scroll","direction":"down","amount":600}
{"type":"wait","ms":500}
{"type":"extract","fields":["title","price","rating","reviews"]}
{"type":"go_back"}
{"type":"reload"}
{"type":"speak","message":"Opening Amazon."}
{"type":"ask_user","question":"Which one?","options":["first","second"]}
{"type":"confirm_sensitive_action","description":"Proceed to payment of ₹4,299?"}
```

Anything else — `{"type":"eval_js"}`, `{"type":"run_shell"}` — fails
deserialization (pinned by tests `unknown_action_type_is_rejected` and
`rejects_unknown_action`).

## Task graph

```jsonc
{
  "goal": "Find a highly-rated mechanical keyboard under ₹5,000",
  "nodes": [
    {
      "id": "navigate",                       // unique, snake_case
      "title": "Navigate",
      "action": {"type":"navigate","url":"https://www.amazon.in"},
      "depends_on": [],
      "success_check": "URL changed to Amazon home or search page"
    }
    // … search → filter → rank → choose
  ],
  "metadata": { "tts": { "navigate": "Opening Amazon." } }
}
```

Validation (`TaskGraph::validate`): non-empty, unique ids, dependencies
resolve, no cycles (Kahn), every node has a non-empty `success_check`.
Execution order is the topological order, stable within ready sets.

## Targeting rules

Prefer accessible **role + name** over CSS selectors. The runtimes honor a
self-healing ladder — role+name → visible text → CSS selector → (vision
fallback, reserved) — so a site changing its markup degrades gracefully
instead of breaking.

## Verification

Every executed action returns `ActionResult` with a `VerificationResult`
(list of labelled checks). `ok` is true only when **all** checks pass; an
empty check list is *not verified* by definition. Failures carry a
`FailureClass` that recovery routes on (see `docs/guardrails.md` and
`browser/src/recovery.rs`).
