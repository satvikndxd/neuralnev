# playwright-worker

The Node sidecar that gives NeuralNav a real browser when `USE_REAL_BROWSER=true`.

## Protocol

JSON lines over stdin/stdout:

```
→ {"id":1,"cmd":"navigate","params":{"url":"https://example.com"}}
← {"id":1,"ok":true,"duration_ms":812,"checks":[...],"page_state":{...}}
```

Supported commands: `navigate`, `click`, `type`, `scroll`, `wait`, `extract`,
`go_back`, `reload`, `get_page_state`, `stop`.

**Structured commands only.** There is no `evaluate` / `exec` command — the
worker refuses anything outside the table above, so a planner (or a compromised
planner) can never inject JavaScript.

## Element resolution — self-healing ladder

`click`/`type` targets resolve in priority order:

1. accessible `role` + `name` (`getByRole`)
2. visible `text` (`getByText`)
3. CSS `selector`
4. bare `name` as visible text (last resort)

## Environment

| Var | Meaning |
| --- | --- |
| `HEADLESS` | `0` to show the browser window (default headless) |
| `ADBLOCK_DOMAINS` | comma-separated domain list; matching requests are aborted |
| `PROXY_URL` | optional proxy server for the browser context |

## Setup

```sh
cd workers/playwright-worker
npm install        # installs playwright + chromium (postinstall)
```

The Rust `PlaywrightSidecarRuntime` spawns this process on demand; you don't
run it manually except to debug (`echo '{"id":1,"cmd":"get_page_state","params":{}}' | node index.js`).
