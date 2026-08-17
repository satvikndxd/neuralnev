/* Pure reducer over TraceEvents → console state. No DOM access, so it is
   unit-testable; views subscribe and re-render the parts that changed. */

import type {
  NeuralNavAction,
  TaskGraph,
  TaskStatus,
  TraceEvent,
  VerificationCheck,
} from "../lib/types";

export interface LogLine {
  kind: "tts" | "verify-ok" | "verify-fail" | "verify-run" | "policy" | "recovery" | "system";
  text: string;
}

export interface ConsoleState {
  sessionId?: string;
  phase: "idle" | "listening" | "planning" | "executing" | "waiting_confirmation" | "done" | "stopped";
  transcript: string;
  transcriptFinal: boolean;
  intent?: { intent: string; confidence: number };
  graph?: TaskGraph;
  nodeStatus: Record<string, TaskStatus>;
  currentAction?: { nodeId: string; action: NeuralNavAction };
  ttsLog: string[];
  verifyLog: LogLine[];
  confirmQuestion?: string;
  success?: boolean;
}

export const initialState: ConsoleState = {
  phase: "idle",
  transcript: "",
  transcriptFinal: false,
  nodeStatus: {},
  ttsLog: [],
  verifyLog: [],
};

const cap = <T>(arr: T[], n: number): T[] => (arr.length > n ? arr.slice(arr.length - n) : arr);

function checksToLines(nodeId: string, checks: VerificationCheck[]): LogLine[] {
  return checks.map((c) => ({
    kind: c.passed ? ("verify-ok" as const) : ("verify-fail" as const),
    text: `${c.label}${c.detail ? ` · ${c.detail}` : ""} (${nodeId})`,
  }));
}

export function reduce(state: ConsoleState, ev: TraceEvent): ConsoleState {
  switch (ev.event) {
    case "session_started":
      return {
        ...initialState,
        sessionId: ev.session_id || state.sessionId,
        phase: "listening",
      };

    case "voice_partial_transcript":
      return { ...state, transcript: ev.text, transcriptFinal: false, phase: "listening" };

    case "voice_final_transcript":
      return { ...state, transcript: ev.text, transcriptFinal: true, phase: "planning" };

    case "intent_parsed":
      return {
        ...state,
        intent: { intent: ev.intent, confidence: ev.confidence },
        verifyLog: cap(
          [
            ...state.verifyLog,
            {
              kind: "verify-ok",
              text: `intent parsed · ${ev.intent} · confidence ${ev.confidence.toFixed(2)}`,
            },
          ],
          8,
        ),
      };

    case "plan_created": {
      const nodeStatus: Record<string, TaskStatus> = {};
      for (const n of ev.graph.nodes) nodeStatus[n.id] = "pending";
      return { ...state, graph: ev.graph, nodeStatus, phase: "executing" };
    }

    case "action_dispatched":
      return {
        ...state,
        phase: "executing",
        currentAction: { nodeId: ev.node_id, action: ev.action },
        nodeStatus: { ...state.nodeStatus, [ev.node_id]: "running" },
        verifyLog: cap(
          [...state.verifyLog, { kind: "verify-run", text: `dispatch ${ev.action.type} → ${ev.node_id}` }],
          8,
        ),
      };

    case "action_verified":
      return {
        ...state,
        nodeStatus: {
          ...state.nodeStatus,
          [ev.node_id]: ev.result.ok ? "success" : "failed",
        },
        verifyLog: cap(
          [...state.verifyLog, ...checksToLines(ev.node_id, ev.result.verification.checks)],
          8,
        ),
      };

    case "action_failed":
      return {
        ...state,
        nodeStatus: { ...state.nodeStatus, [ev.node_id]: "failed" },
        verifyLog: cap(
          [
            ...state.verifyLog,
            { kind: "verify-fail", text: `${ev.error_class}${ev.detail ? ` · ${ev.detail}` : ""} (${ev.node_id})` },
          ],
          8,
        ),
      };

    case "recovery_attempted":
      return {
        ...state,
        verifyLog: cap(
          [...state.verifyLog, { kind: "recovery", text: `recovery: ${ev.strategy} (${ev.node_id})` }],
          8,
        ),
      };

    case "policy_decision":
      if (ev.decision === "allowed") return state;
      return {
        ...state,
        verifyLog: cap(
          [
            ...state.verifyLog,
            { kind: "policy", text: `policy ${ev.decision}${ev.reason ? ` · ${ev.reason}` : ""} (${ev.node_id})` },
          ],
          8,
        ),
      };

    case "user_confirmation_requested":
      return { ...state, phase: "waiting_confirmation", confirmQuestion: ev.question };

    case "user_confirmation_resolved":
      return {
        ...state,
        phase: "executing",
        confirmQuestion: undefined,
        verifyLog: cap(
          [
            ...state.verifyLog,
            { kind: "system", text: ev.approved ? "user approved — continuing" : "user declined — skipping" },
          ],
          8,
        ),
      };

    case "user_stopped":
      return {
        ...state,
        phase: "stopped",
        confirmQuestion: undefined,
        verifyLog: cap(
          [...state.verifyLog, { kind: "verify-fail", text: "barge-in received → all actions halted" }],
          8,
        ),
      };

    case "tts_spoken":
      return { ...state, ttsLog: cap([...state.ttsLog, ev.message], 4) };

    case "session_completed": {
      const phase = state.phase === "stopped" ? "stopped" : "done";
      const nodeStatus = { ...state.nodeStatus };
      if (phase === "stopped") {
        for (const [id, st] of Object.entries(nodeStatus)) {
          if (st === "pending" || st === "running") nodeStatus[id] = "skipped";
        }
      }
      return { ...state, phase, success: ev.success, nodeStatus };
    }
  }
}

/* Tiny subscribe/dispatch wrapper. */
export class Store {
  private state: ConsoleState = initialState;
  private listeners = new Set<(s: ConsoleState) => void>();

  get(): ConsoleState {
    return this.state;
  }

  dispatch(ev: TraceEvent): void {
    this.state = reduce(this.state, ev);
    for (const l of this.listeners) l(this.state);
  }

  replace(state: ConsoleState): void {
    this.state = state;
    for (const l of this.listeners) l(this.state);
  }

  subscribe(fn: (s: ConsoleState) => void): () => void {
    this.listeners.add(fn);
    fn(this.state);
    return () => this.listeners.delete(fn);
  }
}
