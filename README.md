# NeuralNav — Bauhaus UI

A Bauhaus-styled single-page UI for **NeuralNav**, a voice-first autonomous browser
agent: speak a command → structured task graph → MCP browser actions → per-step
verification → concise spoken feedback.

## Run

```sh
python3 -m http.server 3000
# open http://localhost:3000
```

Static site — no build step. `index.html` + `styles.css` + `app.js`.

## Sections

| # | Section | What it shows |
|---|---------|---------------|
| — | Hero | SPEAK. PLAN. VERIFY. + animated Bauhaus grid composition |
| 01 | Live console | Scripted demo trace: voice waveform, typed transcript, task-graph nodes lighting up, structured-action JSON, verifier log, TTS log, barge-in stop |
| 02 | Pipeline | VAD/ASR → Gemini NLU → Planner → Policy → Browser MCP → Verifier, plus the a11y-tree-first / DOM / vision fallback tiers |
| 03 | Metrics | Stat tiles, latency-budget bar chart (with tooltip + table view), task success-rate meters, failure-class → recovery table |
| 04 | Guardrails | Permission levels 1–3 and human-in-the-loop confirmation |

## Design notes

- **Chrome palette:** cream paper `#f2ede1`, ink `#16130e`, primary red / yellow / blue,
  thick rules, geometric shapes (circle / triangle / square), Jost (Futura-style) type.
- **Data palette:** the latency chart uses a single-hue **ordinal blue ramp**
  (`#79aeee → #0c355e`), validated with the dataviz palette validator
  (monotone lightness, adjacent ΔL ≥ 0.06, light-end contrast ≥ 2:1 on the
  `#faf7f0` chart surface — all checks pass). Bars are ≤ 24px with 4px rounded
  data-ends, values at the tip, hairline grid, hover tooltips, and a table view
  for accessibility. Meters below the 90% target are flagged with an
  icon + label, never color alone.
