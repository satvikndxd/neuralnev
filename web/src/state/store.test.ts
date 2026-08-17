import { describe, expect, it } from "vitest";
import type { TaskGraph, TraceEvent } from "../lib/types";
import { initialState, reduce } from "./store";

const graph: TaskGraph = {
  goal: "demo",
  nodes: [
    {
      id: "navigate",
      title: "Navigate",
      action: { type: "navigate", url: "https://www.amazon.in" },
      depends_on: [],
      success_check: "URL changed",
      status: "pending",
      attempts: 0,
    },
    {
      id: "search",
      title: "Search",
      action: { type: "type", role: "textbox", name: "Search", text: "mechanical keyboard" },
      depends_on: ["navigate"],
      success_check: "results visible",
      status: "pending",
      attempts: 0,
    },
  ],
};

const run = (events: TraceEvent[]) => events.reduce(reduce, initialState);

describe("console store reducer", () => {
  it("run-demo flow updates transcript, plan and node statuses", () => {
    const s = run([
      { event: "session_started", session_id: "s1", timestamp: 0 },
      { event: "voice_partial_transcript", text: "Open Amazon" },
      { event: "voice_final_transcript", text: "Open Amazon and find a keyboard" },
      { event: "intent_parsed", intent: "composite_web_task", confidence: 0.94 },
      { event: "plan_created", graph },
      { event: "action_dispatched", node_id: "navigate", action: graph.nodes[0].action },
      {
        event: "action_verified",
        node_id: "navigate",
        result: {
          ok: true,
          action: graph.nodes[0].action,
          verification: { passed: true, checks: [{ label: "URL changed", passed: true }] },
          duration_ms: 500,
        },
      },
    ]);
    expect(s.transcript).toContain("keyboard");
    expect(s.transcriptFinal).toBe(true);
    expect(s.graph?.nodes.length).toBe(2);
    expect(s.nodeStatus.navigate).toBe("success");
    expect(s.nodeStatus.search).toBe("pending");
    expect(s.phase).toBe("executing");
    expect(s.verifyLog.some((l) => l.text.includes("URL changed"))).toBe(true);
  });

  it("stop marks remaining nodes skipped and phase stopped", () => {
    const s = run([
      { event: "session_started", session_id: "s1", timestamp: 0 },
      { event: "plan_created", graph },
      { event: "action_dispatched", node_id: "navigate", action: graph.nodes[0].action },
      { event: "user_stopped", reason: "test" },
      { event: "session_completed", success: false },
    ]);
    expect(s.phase).toBe("stopped");
    expect(s.success).toBe(false);
    expect(s.nodeStatus.navigate).toBe("skipped");
    expect(s.nodeStatus.search).toBe("skipped");
  });

  it("confirmation request parks the run and resolution resumes it", () => {
    const asked = run([
      { event: "session_started", session_id: "s1", timestamp: 0 },
      { event: "plan_created", graph },
      { event: "user_confirmation_requested", question: "Proceed to payment?" },
    ]);
    expect(asked.phase).toBe("waiting_confirmation");
    expect(asked.confirmQuestion).toBe("Proceed to payment?");

    const resumed = reduce(asked, { event: "user_confirmation_resolved", approved: true });
    expect(resumed.phase).toBe("executing");
    expect(resumed.confirmQuestion).toBeUndefined();
  });

  it("tts log is capped at four lines", () => {
    let s = initialState;
    for (let i = 0; i < 9; i++) s = reduce(s, { event: "tts_spoken", message: `line ${i}` });
    expect(s.ttsLog.length).toBe(4);
    expect(s.ttsLog[3]).toBe("line 8");
  });

  it("failure classes and recovery strategies land in the verify log", () => {
    const s = run([
      { event: "session_started", session_id: "s1", timestamp: 0 },
      { event: "plan_created", graph },
      { event: "action_failed", node_id: "search", error_class: "element_not_found" },
      { event: "recovery_attempted", node_id: "search", strategy: "self-heal selector" },
    ]);
    expect(s.nodeStatus.search).toBe("failed");
    expect(s.verifyLog.some((l) => l.text.includes("element_not_found"))).toBe(true);
    expect(s.verifyLog.some((l) => l.text.includes("self-heal"))).toBe(true);
  });
});
