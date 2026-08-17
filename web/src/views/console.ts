/* Live console view: binds the store to the DOM and the controls to the API. */

import { api } from "../lib/api";
import { connectEvents, type ConnectionStatus } from "../lib/sse";
import type { ConsoleState } from "../state/store";
import { initialState, reduce, Store } from "../state/store";
import type { NeuralNavAction, TaskStatus } from "../lib/types";

const statusLabel: Record<TaskStatus, string> = {
  pending: "queued",
  running: "running",
  success: "verified",
  failed: "failed",
  skipped: "skipped",
  waiting_user: "waiting",
};

function actionDetail(action: NeuralNavAction): string {
  switch (action.type) {
    case "navigate":
      return action.url;
    case "click":
      return action.name ?? action.text ?? action.selector ?? "";
    case "type":
      return `"${action.text}"`;
    case "extract":
      return action.fields.join(", ");
    case "scroll":
      return action.direction;
    case "wait":
      return `${action.ms} ms`;
    case "speak":
      return action.message;
    case "ask_user":
      return action.question;
    case "confirm_sensitive_action":
      return action.description;
    default:
      return "";
  }
}

const esc = (s: string) =>
  s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

function jsonHTML(obj: unknown): string {
  return esc(JSON.stringify(obj, null, 2))
    .replace(/&quot;/g, '"')
    .replace(/"([^"]+)":/g, '<span class="j-key">"$1"</span>:')
    .replace(/: "((?:[^"\\]|\\.)*)"/g, ': <span class="j-str">"$1"</span>');
}

export function mountConsole(): void {
  const store = new Store();

  const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
  const wave = $("wave");
  const transcriptText = $("transcript-text");
  const caret = $("caret");
  const goalChip = $("goal-chip");
  const graphList = $("task-graph");
  const actionJson = $("action-json");
  const verifyLog = $("verify-log");
  const ttsLog = $("tts-log");
  const confirmBar = $("confirm-bar");
  const confirmQuestion = $("confirm-question");
  const connBadge = $("conn-badge");
  const runBtn = $<HTMLButtonElement>("run-demo");
  const stopBtn = $<HTMLButtonElement>("barge-in");
  const heroRun = $<HTMLButtonElement>("hero-run");
  const form = $<HTMLFormElement>("command-form");
  const input = $<HTMLInputElement>("command-input");

  let renderedGraphKey = "";

  function renderGraph(state: ConsoleState): void {
    const graph = state.graph;
    if (!graph) {
      if (renderedGraphKey !== "") {
        graphList.innerHTML = "";
        renderedGraphKey = "";
      }
      return;
    }
    const key = graph.goal + graph.nodes.map((n) => n.id).join(",");
    if (key !== renderedGraphKey) {
      renderedGraphKey = key;
      graphList.innerHTML = "";
      graph.nodes.forEach((n, i) => {
        const li = document.createElement("li");
        li.className = "task-node";
        li.id = `node-${n.id}`;
        li.innerHTML =
          `<span class="tn-idx">${i + 1}</span>` +
          `<span class="tn-name">${esc(n.title)}<span class="tn-detail">${esc(actionDetail(n.action))}</span></span>` +
          `<span class="tn-status">queued</span>`;
        graphList.appendChild(li);
      });
    }
    for (const n of graph.nodes) {
      const li = document.getElementById(`node-${n.id}`);
      if (!li) continue;
      const st = state.nodeStatus[n.id] ?? "pending";
      li.dataset.state = st;
      const badge = li.querySelector(".tn-status");
      if (badge) badge.textContent = statusLabel[st];
    }
  }

  function render(state: ConsoleState): void {
    // transcript + waveform
    transcriptText.textContent =
      state.transcript || "Press “Run demo”, or type a command below…";
    caret.hidden = state.phase !== "listening";
    wave.dataset.state = state.phase === "listening" ? "listening" : "idle";

    // goal
    goalChip.textContent = state.graph ? `goal: ${state.graph.goal}` : "goal: —";

    // graph
    renderGraph(state);

    // current structured action
    if (state.currentAction) {
      actionJson.innerHTML = `<code>${jsonHTML(state.currentAction.action)}</code>`;
    } else {
      actionJson.innerHTML =
        "<code>// planner output appears here\n// LLM proposes → validator approves\n// → executor runs → verifier confirms</code>";
    }

    // verifier log
    verifyLog.innerHTML = "";
    for (const line of state.verifyLog) {
      const li = document.createElement("li");
      const cls =
        line.kind === "verify-ok" ? "v-ok"
        : line.kind === "verify-fail" ? "v-fail"
        : line.kind === "policy" ? "v-policy"
        : line.kind === "recovery" ? "v-recovery"
        : "v-run";
      const mark =
        line.kind === "verify-ok" ? "✓"
        : line.kind === "verify-fail" ? "✕"
        : line.kind === "recovery" ? "↻"
        : line.kind === "policy" ? "▲"
        : "…";
      li.innerHTML = `<span class="${cls}">${mark}</span> ${esc(line.text)}`;
      verifyLog.appendChild(li);
    }

    // tts log
    ttsLog.innerHTML = "";
    state.ttsLog.forEach((msg, i) => {
      const li = document.createElement("li");
      if (i === state.ttsLog.length - 1 && (state.phase === "done" || state.phase === "stopped")) {
        li.className = "tts-final";
      }
      li.innerHTML = `${esc(msg)}<em>tts</em>`;
      ttsLog.appendChild(li);
    });

    // confirmation bar
    const waiting = state.phase === "waiting_confirmation" && !!state.confirmQuestion;
    confirmBar.hidden = !waiting;
    if (waiting && state.confirmQuestion) confirmQuestion.textContent = state.confirmQuestion;

    // controls
    const active =
      state.phase === "listening" ||
      state.phase === "planning" ||
      state.phase === "executing" ||
      state.phase === "waiting_confirmation";
    runBtn.disabled = active;
    stopBtn.disabled = !active;
    runBtn.textContent = state.phase === "done" || state.phase === "stopped" ? "↻ Replay demo" : "▶ Run demo";
  }

  store.subscribe(render);

  // ── SSE + snapshot resync ─────────────────────────────────────────
  async function resync(): Promise<void> {
    try {
      const snap = await api.state();
      const badge = document.getElementById("mode-badge");
      if (badge) {
        badge.textContent =
          snap.mode.planner === "mock" && snap.mode.browser === "mock"
            ? "MOCK MODE"
            : `${snap.mode.planner}/${snap.mode.browser}`.toUpperCase();
      }
      // Replay recent history through the reducer to rebuild state.
      if (snap.recent_events.length > 0) {
        store.replace(snap.recent_events.reduce(reduce, initialState));
      }
    } catch {
      /* backend not up yet; badge already shows reconnecting */
    }
  }

  connectEvents(
    (ev) => store.dispatch(ev),
    (status: ConnectionStatus) => {
      connBadge.dataset.state = status;
      connBadge.textContent =
        status === "open" ? "live" : status === "connecting" ? "connecting…" : "reconnecting…";
      if (status === "open") void resync();
    },
  );

  // ── controls ──────────────────────────────────────────────────────
  const start = async (fn: () => Promise<unknown>) => {
    try {
      await fn();
    } catch (e) {
      store.dispatch({
        event: "tts_spoken",
        message: `Backend unreachable (${(e as Error).message}). Is the Rust server running on :4173?`,
      });
    }
  };

  runBtn.addEventListener("click", () => void start(() => api.runDemo()));
  heroRun.addEventListener("click", () => {
    document.getElementById("console")?.scrollIntoView({ behavior: "smooth" });
    void start(() => api.runDemo());
  });
  stopBtn.addEventListener("click", () => void start(() => api.stop("user pressed stop")));
  form.addEventListener("submit", (e) => {
    e.preventDefault();
    const text = input.value.trim();
    if (!text) return;
    input.value = "";
    void start(() => api.runText(text));
  });
  $("confirm-yes").addEventListener("click", () => void start(() => api.confirm(true)));
  $("confirm-no").addEventListener("click", () => void start(() => api.confirm(false)));

  void resync();
}
