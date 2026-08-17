/* Mirrors of crates/core serde types (hand-maintained; see ASSUMPTIONS.md —
   ts-rs generation was skipped in favor of this single small file). */

export type ScrollDirection = "up" | "down";

export type NeuralNavAction =
  | { type: "navigate"; url: string }
  | { type: "click"; selector?: string; role?: string; name?: string; text?: string }
  | { type: "type"; selector?: string; role?: string; name?: string; text: string }
  | { type: "scroll"; direction: ScrollDirection; amount?: number }
  | { type: "wait"; ms: number }
  | { type: "extract"; fields: string[] }
  | { type: "go_back" }
  | { type: "reload" }
  | { type: "speak"; message: string }
  | { type: "ask_user"; question: string; options?: string[] }
  | { type: "confirm_sensitive_action"; description: string };

export type TaskStatus =
  | "pending"
  | "running"
  | "success"
  | "failed"
  | "skipped"
  | "waiting_user";

export interface TaskNode {
  id: string;
  title: string;
  action: NeuralNavAction;
  depends_on: string[];
  success_check: string;
  status: TaskStatus;
  attempts: number;
  last_error?: string;
}

export interface TaskGraph {
  goal: string;
  nodes: TaskNode[];
  metadata?: Record<string, unknown>;
}

export interface VerificationCheck {
  label: string;
  passed: boolean;
  detail?: string;
}

export interface VerificationResult {
  passed: boolean;
  checks: VerificationCheck[];
}

export interface PageState {
  url: string;
  title: string;
  page_type?: string;
  result_count?: number;
  loading: boolean;
}

export type FailureClass =
  | "element_not_found"
  | "page_timeout"
  | "popup_blocking_view"
  | "auth_required"
  | "captcha_required"
  | "ambiguous_command"
  | "network_error"
  | "action_verification_failed"
  | "policy_blocked"
  | "unknown";

export interface ActionResult {
  ok: boolean;
  action: NeuralNavAction;
  verification: VerificationResult;
  page_state?: PageState;
  error_class?: FailureClass;
  duration_ms: number;
  extracted?: unknown;
}

export type TraceEvent =
  | { event: "session_started"; session_id: string; timestamp: number }
  | { event: "voice_partial_transcript"; text: string }
  | { event: "voice_final_transcript"; text: string }
  | { event: "intent_parsed"; intent: string; confidence: number }
  | { event: "plan_created"; graph: TaskGraph }
  | { event: "action_dispatched"; node_id: string; action: NeuralNavAction }
  | { event: "action_verified"; node_id: string; result: ActionResult }
  | { event: "action_failed"; node_id: string; error_class: FailureClass; detail?: string }
  | { event: "recovery_attempted"; node_id: string; strategy: string }
  | { event: "policy_decision"; node_id: string; decision: string; reason?: string }
  | { event: "user_confirmation_requested"; question: string }
  | { event: "user_confirmation_resolved"; approved: boolean }
  | { event: "user_stopped"; reason?: string }
  | { event: "tts_spoken"; message: string }
  | { event: "session_completed"; success: boolean };

export interface ServerSnapshot {
  session_id?: string;
  status: "idle" | "running" | "waiting_confirmation" | "completed" | "stopped";
  transcript?: string;
  graph?: TaskGraph;
  last_success?: boolean;
  awaiting_confirmation: boolean;
  recent_events: TraceEvent[];
  mode: {
    planner: string;
    browser: string;
    asr: string;
    tts: string;
    permission_level: string;
  };
}
