import { describe, expect, it } from "vitest";
import { toMonitorSnapshot } from "./monitorApi";

describe("monitor snapshot boundary", () => {
  it("maps only the explicit metadata contract", () => {
    const snapshot = toMonitorSnapshot({
      generated_at_ms: 10,
      agents: [
        {
          thread_id: "child",
          parent_thread_id: "root",
          agent_path: "/root/review",
          display_name: "Review",
          role: "worker",
          project: "project",
          originator: "Codex Desktop",
          requested_model: "gpt-5.6-sol",
          effective_model: "gpt-5.6-terra",
          model_source: "turn-context",
          reasoning_effort: "high",
          status: "running",
          model_drift: true,
          is_subagent: true,
          depth: 1,
          started_at_ms: 1,
          updated_at_ms: 9,
          freshness_ms: 1,
          conversation: "CANARY_SECRET",
          tool_output: "CANARY_OUTPUT",
        },
      ],
      counts: { roots: 1, subagents: 1, running: 1 },
      health: {
        state_database: { level: "healthy", message: "ready", error_count: 0 },
        rollout_observer: { level: "healthy", message: "ready", error_count: 0 },
      },
      raw_path: "C:/CANARY/PATH",
    });

    expect(snapshot.agents[0].effective_model).toBe("gpt-5.6-terra");
    expect(snapshot.agents[0].requested_model).toBe("gpt-5.6-sol");
    expect(snapshot.agents[0].status).toBe("running");
    expect(JSON.stringify(snapshot)).not.toContain("CANARY");
  });

  it("falls back to safe status values for malformed input", () => {
    const snapshot = toMonitorSnapshot({
      agents: [{ thread_id: "child", display_name: "Child", status: "mystery" }],
      health: {},
    });
    expect(snapshot.agents[0].status).toBe("tracking-error");
    expect(snapshot.health.state_database.level).toBe("error");
  });
});
