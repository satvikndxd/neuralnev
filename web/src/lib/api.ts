import type { ServerSnapshot } from "./types";

async function post<T = unknown>(path: string, body?: unknown): Promise<T> {
  const res = await fetch(path, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body ?? {}),
  });
  if (!res.ok && res.status !== 409) {
    throw new Error(`${path} → HTTP ${res.status}`);
  }
  return (await res.json().catch(() => ({}))) as T;
}

export const api = {
  state: async (): Promise<ServerSnapshot> => {
    const res = await fetch("/api/state");
    if (!res.ok) throw new Error(`state → HTTP ${res.status}`);
    return res.json();
  },
  startSession: () => post<{ session_id: string }>("/api/session/start"),
  runDemo: () => post("/api/command", { demo: true }),
  runText: (text: string) => post("/api/command", { text }),
  runVoice: () => post("/api/command", { audio: true }),
  stop: (reason?: string) => post("/api/stop", { reason }),
  confirm: (approved: boolean) => post("/api/confirm", { approved }),
};
