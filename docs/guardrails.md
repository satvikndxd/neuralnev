# Guardrails

Every action passes through `PolicyEngine::evaluate(action, level)` before
dispatch. The engine returns one of:

- `Allowed`
- `Blocked { reason }` — emitted as `PolicyDecision` + `ActionFailed
  (policy_blocked)`; the node fails, dependents are skipped.
- `ConfirmationRequired { description }` — the run parks on
  `UserConfirmationRequested` until `POST /api/confirm {approved}` (deny
  skips the node) or stop.

## Permission levels

| | Level 1 · Read-only | Level 2 · Interactive | Level 3 · Restricted |
| --- | --- | --- | --- |
| navigate / scroll / extract / go_back / reload | ✓ | ✓ | ✓ |
| click / type | ✗ blocked | ✓ | ✓ |
| payment / checkout / send / delete / download shaped actions | ✗ blocked (mutating) | confirmation | confirmation |
| `confirm_sensitive_action` node | confirmation | confirmation | confirmation |

Set via `PERMISSION_LEVEL=read_only|interactive|restricted` (default
`interactive`).

## Sensitivity detection

Keyword scan over the action's textual surface (name/text/url/description):
`checkout`, `payment`, `buy now`, `place order`, `delete`, `send message`,
`download`, `transfer`, … (full list in `guardrails/src/policy.rs`). The
explicit `confirm_sensitive_action` node *always* requires confirmation at
every level.

## Hard rules

- **CAPTCHAs are never automated.** Any action whose target mentions
  CAPTCHA is `Blocked` at every level; at runtime a `captcha_required`
  failure pauses for the human (recovery `PauseForHuman`) — never bypassed.
- **Payments are never automatic.** There is no path to a payment-shaped
  action that does not emit `UserConfirmationRequested` first.
- **Stop always wins.** `/api/stop` cancels the run token, hard-stops TTS
  mid-word, and resolves any pending confirmation as denied.

## Failure → recovery routing

| FailureClass | Strategy |
| --- | --- |
| `element_not_found` | self-heal selector ladder ×2 → abort |
| `page_timeout` | retry ×2 → abort |
| `popup_blocking_view` | dismiss popups, retry |
| `auth_required` | pause for human |
| `captcha_required` | pause for human (never bypass) |
| `ambiguous_command` | ask clarification |
| `network_error` | proxy health-check + failover ×2 → abort |
| `action_verification_failed` | retry ×2 → abort |
| `policy_blocked` | abort (no retry) |
| `unknown` | retry ×1 → abort |

Each attempt emits `RecoveryAttempted { strategy }` so the trace shows *why*
the agent did what it did.
