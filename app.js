/* NeuralNav — Bauhaus UI
   Charts follow the dataviz method:
   - latency budget = ordinal ramp (one hue, monotone lightness), validated:
     #79aeee #5093e6 #2a78d6 #1e5dae #154a85 #0c355e  → ALL CHECKS PASS (light, surface #faf7f0)
   - bars ≤24px, 4px rounded data-end (square at baseline), hairline grids,
     value at the tip, hover tooltip, table view toggle.
*/
(() => {
  "use strict";

  const $ = (sel, root = document) => root.querySelector(sel);

  /* ════════════ Latency budget chart ════════════ */
  const LATENCY = [
    { stage: "Voice capture + VAD",      ms: 150 },
    { stage: "Streaming ASR partial",    ms: 400 },
    { stage: "Intent classification",    ms: 300 },
    { stage: "Action planning",          ms: 500 },
    { stage: "Browser dispatch",         ms: 200 },
    { stage: "First spoken feedback",    ms: 150 },
  ];
  const RAMP = ["#79aeee", "#5093e6", "#2a78d6", "#1e5dae", "#154a85", "#0c355e"];
  const TOTAL = LATENCY.reduce((s, d) => s + d.ms, 0);

  function renderLatencyChart() {
    const host = $("#latency-chart");
    if (!host) return;

    const W = 640, LM = 168, RM = 56, TM = 8, BM = 26;
    const rowH = 34, barH = 18, r = 4;
    const H = TM + LATENCY.length * rowH + BM;
    const plotW = W - LM - RM;
    const max = 500;
    const x = v => LM + (v / max) * plotW;

    const NS = "http://www.w3.org/2000/svg";
    const svg = document.createElementNS(NS, "svg");
    svg.setAttribute("viewBox", `0 0 ${W} ${H}`);
    svg.setAttribute("aria-hidden", "true");

    const el = (tag, attrs, text) => {
      const n = document.createElementNS(NS, tag);
      for (const k in attrs) n.setAttribute(k, attrs[k]);
      if (text != null) n.textContent = text;
      return n;
    };

    // hairline gridlines + ticks (0–500 by 100)
    for (let v = 0; v <= max; v += 100) {
      svg.appendChild(el("line", {
        x1: x(v), x2: x(v), y1: TM, y2: TM + LATENCY.length * rowH,
        stroke: v === 0 ? "#b9b3a0" : "#e1ddd0", "stroke-width": 1,
      }));
      svg.appendChild(el("text", {
        x: x(v), y: H - 8, "text-anchor": "middle",
        "font-size": 11, fill: "#8a8474",
        style: "font-variant-numeric: tabular-nums",
      }, String(v)));
    }

    LATENCY.forEach((d, i) => {
      const y = TM + i * rowH + (rowH - barH) / 2;
      const w = x(d.ms) - x(0);

      // stage label (text token, never the series color)
      svg.appendChild(el("text", {
        x: LM - 10, y: y + barH / 2 + 4, "text-anchor": "end",
        "font-size": 12.5, fill: "#55503f", "font-weight": 500,
      }, d.stage));

      // bar: square at baseline, 4px rounded data-end
      svg.appendChild(el("path", {
        d: `M${x(0)},${y} h${w - r} a${r},${r} 0 0 1 ${r},${r} v${barH - 2 * r} a${r},${r} 0 0 1 ${-r},${r} h${-(w - r)} Z`,
        fill: RAMP[i],
      }));

      // value at the tip
      svg.appendChild(el("text", {
        x: x(d.ms) + 7, y: y + barH / 2 + 4,
        "font-size": 12, fill: "#16130e", "font-weight": 600,
        style: "font-variant-numeric: tabular-nums",
      }, d.ms + " ms"));

      // hover hit target — full row, larger than the mark
      const hit = el("rect", {
        x: 0, y: TM + i * rowH, width: W, height: rowH,
        fill: "transparent", class: "bar-hit",
      });
      hit.dataset.idx = i;
      svg.appendChild(hit);
    });

    host.appendChild(svg);

    // tooltip layer
    const tip = $("#viz-tooltip");
    svg.addEventListener("pointermove", e => {
      const t = e.target.closest(".bar-hit");
      if (!t) { tip.hidden = true; return; }
      const d = LATENCY[+t.dataset.idx];
      tip.innerHTML = `<b>${d.stage}</b><span class="tt-num">${d.ms} ms · ${Math.round((d.ms / TOTAL) * 100)}% of ${TOTAL.toLocaleString()} ms budget</span>`;
      tip.hidden = false;
      const pad = 14;
      let tx = e.clientX + pad, ty = e.clientY + pad;
      const r2 = tip.getBoundingClientRect();
      if (tx + r2.width > innerWidth - 8) tx = e.clientX - r2.width - pad;
      if (ty + r2.height > innerHeight - 8) ty = e.clientY - r2.height - pad;
      tip.style.left = tx + "px";
      tip.style.top = ty + "px";
    });
    svg.addEventListener("pointerleave", () => { tip.hidden = true; });

    // table view
    const tbody = $("#latency-table tbody");
    LATENCY.forEach(d => {
      const tr = document.createElement("tr");
      tr.innerHTML = `<td>${d.stage}</td><td>${d.ms}</td><td>${Math.round((d.ms / TOTAL) * 100)}%</td>`;
      tbody.appendChild(tr);
    });
    const toggle = $("#latency-toggle"), table = $("#latency-table");
    toggle.addEventListener("click", () => {
      const showTable = table.hidden;
      table.hidden = !showTable;
      host.style.display = showTable ? "none" : "";
      toggle.setAttribute("aria-pressed", String(showTable));
      toggle.textContent = showTable ? "Chart view" : "Table view";
    });
  }

  /* ════════════ Success-rate meters ════════════ */
  const SUCCESS = [
    { name: "Navigation",           rate: 96 },
    { name: "Search & filter",      rate: 91 },
    { name: "Extraction",           rate: 88 },
    { name: "Multi-step composite", rate: 74 },
  ];
  const TARGET = 90;

  function renderMeters() {
    const host = $("#meters");
    if (!host) return;
    SUCCESS.forEach(d => {
      const below = d.rate < TARGET;
      const row = document.createElement("div");
      row.className = "meter";
      row.innerHTML =
        `<span class="meter-name">${d.name}${below ? '<span class="warn-flag">⚠ below target</span>' : ""}</span>` +
        `<span class="meter-val">${d.rate}%</span>` +
        `<div class="meter-track${below ? " warn" : ""}" role="meter" aria-valuenow="${d.rate}" aria-valuemin="0" aria-valuemax="100" aria-label="${d.name} success rate">` +
        `<span class="meter-fill" style="width:${d.rate}%"></span>` +
        `<span class="meter-target" style="left:${TARGET}%" title="90% target"></span>` +
        `</div>`;
      host.appendChild(row);
    });
  }

  /* ════════════ Console demo ════════════ */
  const COMMAND = "Open Amazon and find a mechanical keyboard under 5,000 rupees with good reviews.";
  const GOAL = "Find a highly-rated mechanical keyboard under ₹5,000";

  const STEPS = [
    {
      id: "open_site", name: "Navigate", detail: "amazon.in",
      tts: "Opening Amazon.",
      action: { type: "navigate", url: "https://www.amazon.in" },
      checks: ["URL changed → amazon.in", "network idle · 212 ms", "DOM ready"],
    },
    {
      id: "search_item", name: "Search", detail: 'query: "mechanical keyboard"',
      tts: "Searching for keyboards.",
      action: { type: "type", target: { role: "searchbox", name: "Search Amazon.in" }, text: "mechanical keyboard", submit: true },
      checks: ["results list appeared", "page title matches query"],
    },
    {
      id: "apply_price_filter", name: "Filter", detail: "price < ₹5,000",
      tts: "Filtering under five thousand.",
      action: { type: "click", target: { role: "link", name: "Under ₹5,000" }, fallback: ["text=Under ₹5,000", "css=#p_36 a"] },
      checks: ["result count 1,204 → 312", "filter chip visible"],
    },
    {
      id: "rank_results", name: "Rank", detail: "criteria: rating · reviews ≥ 500",
      tts: "Ranking by rating.",
      action: { type: "extract", fields: ["title", "price", "rating", "review_count"], limit: 24 },
      checks: ["24 cards extracted", "schema valid"],
    },
    {
      id: "select_best", name: "Choose best", detail: "highest rated with enough reviews",
      tts: "Done. Top pick — ₹4,299, rated 4.6 stars by 2,143 reviewers.",
      final: true,
      action: { type: "choose_result", policy: "max(rating) where review_count >= 500", result: { title: "Cosmic Byte CB-GK-26", price: "₹4,299", rating: 4.6, reviews: 2143 } },
      checks: ["product page opened", "success_criteria met ✓"],
    },
  ];

  const state = { running: false, timers: [] };
  const later = (fn, ms) => state.timers.push(setTimeout(fn, ms));
  const clearTimers = () => { state.timers.forEach(clearTimeout); state.timers = []; };

  const waveEl = $("#wave"), transcriptEl = $("#transcript-text"),
        ttsLog = $("#tts-log"), graphEl = $("#task-graph"),
        goalChip = $("#goal-chip"), actionJson = $("#action-json"),
        verifyLog = $("#verify-log"),
        runBtn = $("#run-demo"), heroRun = $("#hero-run"), stopBtn = $("#barge-in");

  function jsonHTML(obj) {
    const json = JSON.stringify(obj, null, 2);
    return json
      .replace(/&/g, "&amp;").replace(/</g, "&lt;")
      .replace(/"([^"]+)":/g, '<span class="j-key">"$1"</span>:')
      .replace(/: "((?:[^"\\]|\\.)*)"/g, ': <span class="j-str">"$1"</span>');
  }

  function speak(text, final = false) {
    const li = document.createElement("li");
    if (final) li.className = "tts-final";
    li.innerHTML = `${text}<em>tts</em>`;
    ttsLog.appendChild(li);
    while (ttsLog.children.length > 4) ttsLog.removeChild(ttsLog.firstChild);
  }

  function verify(text, cls) {
    const li = document.createElement("li");
    const mark = cls === "v-ok" ? "✓" : cls === "v-fail" ? "✕" : "…";
    li.innerHTML = `<span class="${cls}">${mark}</span> ${text}`;
    verifyLog.appendChild(li);
    while (verifyLog.children.length > 5) verifyLog.removeChild(verifyLog.firstChild);
  }

  function buildGraph() {
    graphEl.innerHTML = "";
    STEPS.forEach((s, i) => {
      const li = document.createElement("li");
      li.className = "task-node";
      li.dataset.state = "pending";
      li.id = "node-" + s.id;
      li.innerHTML =
        `<span class="tn-idx">${i + 1}</span>` +
        `<span class="tn-name">${s.name}<span class="tn-detail">${s.detail}</span></span>` +
        `<span class="tn-status">queued</span>`;
      graphEl.appendChild(li);
    });
  }

  function setNode(id, st, label) {
    const n = $("#node-" + id);
    if (!n) return;
    n.dataset.state = st;
    n.querySelector(".tn-status").textContent = label;
  }

  function typeTranscript(text, done) {
    transcriptEl.textContent = "";
    let i = 0;
    const tick = () => {
      if (!state.running) return;
      transcriptEl.textContent = text.slice(0, ++i);
      if (i < text.length) later(tick, 26);
      else later(done, 350);
    };
    tick();
  }

  function runStep(idx) {
    if (!state.running) return;
    if (idx >= STEPS.length) return finish();
    const s = STEPS[idx];

    setNode(s.id, "running", "running");
    actionJson.innerHTML = `<code>${jsonHTML(s.action)}</code>`;
    speak(s.tts, !!s.final);
    verify(`dispatch ${s.action.type} → ${s.id}`, "v-run");

    s.checks.forEach((c, ci) => later(() => verify(c, "v-ok"), 420 + ci * 340));

    later(() => {
      if (!state.running) return;
      setNode(s.id, "done", "verified");
      runStep(idx + 1);
    }, 520 + s.checks.length * 340 + 420);
  }

  function finish() {
    state.running = false;
    waveEl.dataset.state = "idle";
    stopBtn.disabled = true;
    runBtn.disabled = false;
    runBtn.textContent = "↻ Replay demo";
  }

  function startDemo() {
    if (state.running) return;
    clearTimers();
    state.running = true;
    runBtn.disabled = true;
    stopBtn.disabled = false;
    ttsLog.innerHTML = "";
    verifyLog.innerHTML = "";
    goalChip.textContent = "goal: —";
    actionJson.innerHTML = "<code><span class='j-com'>// listening…</span></code>";
    buildGraph();
    waveEl.dataset.state = "listening";

    typeTranscript(COMMAND, () => {
      waveEl.dataset.state = "idle";
      goalChip.textContent = "goal: " + GOAL;
      verify("intent parsed · confidence 0.94 · no clarification needed", "v-ok");
      later(() => runStep(0), 500);
    });
  }

  function bargeIn() {
    if (!state.running) return;
    clearTimers();
    state.running = false;
    document.querySelectorAll('.task-node[data-state="running"], .task-node[data-state="pending"]').forEach(n => {
      n.dataset.state = "stopped";
      n.querySelector(".tn-status").textContent = "stopped";
    });
    verify("barge-in received → all actions halted", "v-fail");
    speak("Stopped.", true);
    finish();
  }

  runBtn.addEventListener("click", startDemo);
  stopBtn.addEventListener("click", bargeIn);
  heroRun.addEventListener("click", () => {
    $("#console").scrollIntoView({ behavior: "smooth" });
    later(startDemo, 450);
  });

  /* ════════════ init ════════════ */
  renderLatencyChart();
  renderMeters();
  buildGraph();
})();
