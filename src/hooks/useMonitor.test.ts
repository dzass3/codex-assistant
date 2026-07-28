import { renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { MonitorSnapshot } from "../../shared/monitor-types";
import { monitorApi } from "../lib/monitorApi";
import { useMonitor } from "./useMonitor";

vi.mock("../lib/monitorApi", () => ({
  monitorApi: {
    getSnapshot: vi.fn(),
    getSettings: vi.fn(),
    subscribe: vi.fn(),
    refresh: vi.fn(),
    setCodexHome: vi.fn(),
  },
}));

const snapshot: MonitorSnapshot = {
  generated_at_ms: 10,
  codex_running: true,
  session_started_at_ms: 1,
  observer_status: "live",
  agents: [],
  counts: {
    roots: 0,
    subagents: 0,
    starting: 0,
    running: 0,
    uncertain: 0,
    historical_unclosed: 0,
    idle: 0,
    interrupted: 0,
    tracking_errors: 0,
    model_drifts: 0,
  },
  health: {
    state_database: {
      level: "healthy",
      message: "ready",
      last_success_ms: 9,
      error_count: 0,
    },
    rollout_observer: {
      level: "healthy",
      message: "ready",
      last_success_ms: 9,
      error_count: 0,
    },
  },
};

describe("useMonitor", () => {
  beforeEach(() => vi.clearAllMocks());

  it("loads the read-only observer and releases its event subscription", async () => {
    const stop = vi.fn();
    vi.mocked(monitorApi.getSnapshot).mockResolvedValue(snapshot);
    vi.mocked(monitorApi.getSettings).mockResolvedValue({
      codex_home_label: "~/.codex",
      is_default: true,
    });
    vi.mocked(monitorApi.subscribe).mockResolvedValue(stop);

    const { result, unmount } = renderHook(() => useMonitor());

    expect(result.current.loading).toBe(true);
    await waitFor(() => expect(result.current.connected).toBe(true));
    expect(result.current.snapshot).toEqual(snapshot);
    expect(result.current.settings).toEqual({ codex_home_label: "~/.codex", is_default: true });

    unmount();
    expect(stop).toHaveBeenCalledOnce();
  });
});
