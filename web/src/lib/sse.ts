/* Reconnecting SSE client. On (re)connect the consumer refetches /api/state
   to heal any gap; backoff grows 1s → 2s → 5s and resets on success. */

import type { TraceEvent } from "./types";

export type ConnectionStatus = "connecting" | "open" | "reconnecting";

export interface SseHandle {
  close(): void;
}

const EVENT_NAMES = [
  "session_started",
  "voice_partial_transcript",
  "voice_final_transcript",
  "intent_parsed",
  "plan_created",
  "action_dispatched",
  "action_verified",
  "action_failed",
  "recovery_attempted",
  "policy_decision",
  "user_confirmation_requested",
  "user_confirmation_resolved",
  "user_stopped",
  "tts_spoken",
  "session_completed",
  "lagged",
];

export function connectEvents(
  onEvent: (ev: TraceEvent) => void,
  onStatus: (status: ConnectionStatus) => void,
): SseHandle {
  let source: EventSource | null = null;
  let closed = false;
  let attempt = 0;
  let timer: number | undefined;

  const open = () => {
    if (closed) return;
    onStatus(attempt === 0 ? "connecting" : "reconnecting");
    source = new EventSource("/api/events");

    source.onopen = () => {
      attempt = 0;
      onStatus("open");
    };

    for (const name of EVENT_NAMES) {
      source.addEventListener(name, (e) => {
        try {
          const data = JSON.parse((e as MessageEvent).data);
          if (name === "lagged") {
            // Dropped events server-side; consumer resyncs via /api/state.
            onEvent({ event: "session_started", session_id: "", timestamp: 0 });
            return;
          }
          onEvent(data as TraceEvent);
        } catch {
          /* ignore malformed frames */
        }
      });
    }

    source.onerror = () => {
      source?.close();
      source = null;
      if (closed) return;
      attempt += 1;
      onStatus("reconnecting");
      const delay = attempt === 1 ? 1000 : attempt === 2 ? 2000 : 5000;
      timer = window.setTimeout(open, delay);
    };
  };

  open();
  return {
    close() {
      closed = true;
      window.clearTimeout(timer);
      source?.close();
    },
  };
}
