import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "./invoke";
import { listen } from "./listen";
import { monitorApi, toMonitorSnapshot } from "./monitorApi";

vi.mock("./invoke", () => ({ invoke: vi.fn() }));
vi.mock("./listen", () => ({ listen: vi.fn() }));

describe("monitor snapshot boundary", () => {
  it("projects only the approved metadata contract", () => {
    const snapshot = toMonitorSnapshot({
      generated_at_ms: 10,
      agents: [
        {
          thread_id: "root",
          parent_thread_id: null,
          display_name: "Root task",
          role: null,
          project: "sample-project",
          originator: "Codex Desktop",
          requested_model: null,
          effective_model: "gpt-5.6-sol",
          model_source: "turn-context",
          reasoning_effort: "xhigh",
          status: "running",
          model_drift: false,
          is_subagent: false,
          depth: 0,
          started_at_ms: 1,
          updated_at_ms: 9,
          freshness_ms: 1,
          prompt: "CANARY_PRIVATE_PROMPT",
          tool_output: "CANARY_PRIVATE_TOOL_OUTPUT",
        },
        {
          thread_id: "child",
          parent_thread_id: "root",
          display_name: "Review",
          role: "reviewer",
          project: "sample-project",
          originator: "Codex Desktop",
          requested_model: "gpt-5.6-sol",
          effective_model: "gpt-5.6-terra",
          model_source: "turn-context",
          reasoning_effort: "high",
          status: "running",
          model_drift: true,
          is_subagent: true,
          depth: 1,
          started_at_ms: 2,
          updated_at_ms: 9,
          freshness_ms: 1,
          response: "CANARY_PRIVATE_RESPONSE",
          full_path: "C:\\private\\sample-project",
        },
      ],
      counts: {
        roots: 1,
        subagents: 1,
        starting: 0,
        running: 2,
        idle: 0,
        interrupted: 0,
        tracking_errors: 0,
        model_drifts: 1,
      },
      health: {
        state_database: {
          level: "healthy",
          message: "ready",
          last_success_ms: 8,
          error_count: 0,
        },
        rollout_observer: {
          level: "healthy",
          message: "ready",
          last_success_ms: 9,
          error_count: 0,
        },
      },
      raw_path: "C:\\private\\state_5.sqlite",
    });

    expect(snapshot.agents).toHaveLength(2);
    expect(snapshot.agents[1]).toMatchObject({
      thread_id: "child",
      parent_thread_id: "root",
      requested_model: "gpt-5.6-sol",
      effective_model: "gpt-5.6-terra",
      model_source: "turn-context",
      model_drift: true,
    });
    expect(JSON.stringify(snapshot)).not.toContain("CANARY");
    expect(JSON.stringify(snapshot)).not.toContain("C:\\private");
  });

  it("replaces backend health details with bounded user-facing status copy", () => {
    const snapshot = toMonitorSnapshot({
      health: {
        state_database: {
          level: "error",
          message: "C:\\Users\\Alice\\.codex\\state_5.sqlite: access denied",
          error_count: 1,
        },
        rollout_observer: {
          level: "degraded",
          message: "D:\\private\\sessions scan failed",
          error_count: 2,
        },
      },
    });

    expect(snapshot.health.state_database.message).toBe("状态数据库暂时不可用");
    expect(snapshot.health.rollout_observer.message).toBe("运行记录观察器数据可能不完整");
    expect(JSON.stringify(snapshot)).not.toContain("Alice");
    expect(JSON.stringify(snapshot)).not.toContain("private");
  });

  it("never upgrades requested-only intent into an effective model", () => {
    const snapshot = toMonitorSnapshot({
      agents: [
        {
          thread_id: "pending-child",
          display_name: "Pending child",
          requested_model: "gpt-5.6-terra",
          effective_model: "gpt-5.6-sol",
          model_source: "requested-only",
          status: "mystery",
          model_drift: true,
          depth: -4,
        },
        { display_name: "Missing identity" },
      ],
      counts: { running: -2, subagents: 1.5 },
      health: {
        state_database: { level: "unknown", error_count: -3 },
        rollout_observer: {},
      },
    });

    expect(snapshot.agents).toHaveLength(1);
    expect(snapshot.agents[0]).toMatchObject({
      requested_model: "gpt-5.6-terra",
      effective_model: null,
      model_source: "requested-only",
      status: "tracking-error",
      model_drift: false,
      depth: 0,
    });
    expect(snapshot.counts.running).toBe(0);
    expect(snapshot.counts.subagents).toBe(0);
    expect(snapshot.health.state_database).toMatchObject({ level: "error", error_count: 0 });
  });
});

describe("monitor command surface", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(listen).mockReset();
  });

  it("uses only read-only observer commands and the namespaced snapshot event", async () => {
    const wireSnapshot = {
      generated_at_ms: 10,
      agents: [],
      counts: {},
      health: {},
    };
    const settings = { codex_home_label: "Default Codex home", is_default: true };
    const stop = vi.fn();
    const onSnapshot = vi.fn();

    vi.mocked(invoke)
      .mockResolvedValueOnce(wireSnapshot)
      .mockResolvedValueOnce(wireSnapshot)
      .mockResolvedValueOnce(settings)
      .mockResolvedValueOnce(settings);
    vi.mocked(listen).mockImplementationOnce(async (event, handler) => {
      expect(event).toBe("monitor://snapshot");
      handler({ payload: wireSnapshot } as never);
      return stop;
    });

    await expect(monitorApi.getSnapshot()).resolves.toMatchObject({ agents: [] });
    await expect(monitorApi.refresh()).resolves.toMatchObject({ agents: [] });
    await expect(monitorApi.getSettings()).resolves.toEqual(settings);
    await expect(monitorApi.setCodexHome("C:\\Users\\User\\.codex")).resolves.toEqual(settings);
    await expect(monitorApi.subscribe(onSnapshot)).resolves.toBe(stop);

    expect(invoke).toHaveBeenNthCalledWith(1, "get_monitor_snapshot");
    expect(invoke).toHaveBeenNthCalledWith(2, "refresh_monitor");
    expect(invoke).toHaveBeenNthCalledWith(3, "get_monitor_settings");
    expect(invoke).toHaveBeenNthCalledWith(4, "set_codex_home", {
      path: "C:\\Users\\User\\.codex",
    });
    expect(onSnapshot).toHaveBeenCalledWith(expect.objectContaining({ agents: [] }));
  });
});
