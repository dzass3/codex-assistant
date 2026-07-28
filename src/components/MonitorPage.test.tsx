import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { MonitorSnapshot } from "../../shared/monitor-types";
import { useMonitor } from "../hooks/useMonitor";
import { MonitorPage } from "./MonitorPage";

vi.mock("../hooks/useMonitor", () => ({ useMonitor: vi.fn() }));

const snapshot: MonitorSnapshot = {
  generated_at_ms: 10,
  codex_running: true,
  session_started_at_ms: 1,
  observer_status: "live",
  agents: [
    {
      thread_id: "root",
      parent_thread_id: null,
      display_name: "主任务",
      role: null,
      project: "assistant",
      originator: "Codex Desktop",
      requested_model: "gpt-5.6-sol",
      effective_model: "gpt-5.6-sol",
      model_source: "turn-context",
      reasoning_effort: "high",
      status: "idle",
      model_drift: false,
      is_subagent: false,
      depth: 0,
      started_at_ms: 1,
      updated_at_ms: 9,
      freshness_ms: 1,
    },
    {
      thread_id: "child",
      parent_thread_id: "root",
      display_name: "安全审查",
      role: "reviewer",
      project: "assistant",
      originator: "Codex Desktop",
      requested_model: "gpt-5.6-terra",
      effective_model: "gpt-5.6-terra",
      model_source: "turn-context",
      reasoning_effort: "xhigh",
      status: "running",
      model_drift: false,
      is_subagent: true,
      depth: 1,
      started_at_ms: 2,
      updated_at_ms: 9,
      freshness_ms: 1,
    },
  ],
  counts: {
    roots: 1,
    subagents: 1,
    starting: 0,
    running: 1,
    uncertain: 0,
    historical_unclosed: 0,
    idle: 1,
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

describe("MonitorPage", () => {
  beforeEach(() => {
    vi.mocked(useMonitor).mockReturnValue({
      snapshot,
      settings: { codex_home_label: "~/.codex", is_default: true },
      loading: false,
      refreshing: false,
      error: null,
      connected: true,
      refresh: vi.fn(),
      setCodexHome: vi.fn(),
    });
  });

  it("shows the active read-only hierarchy and source health without routing controls", () => {
    render(<MonitorPage />);

    expect(screen.getByRole("heading", { name: "当前任务与子代理" })).toBeInTheDocument();
    expect(screen.getByText("安全审查")).toBeInTheDocument();
    expect(screen.getByText("监控数据源正常")).toBeInTheDocument();
    expect(screen.getByText("只读元数据 · 不采集对话内容")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "立即刷新" })).toBeEnabled();
    expect(screen.queryByText(/Smart Routing/i)).not.toBeInTheDocument();
  });

  it("keeps the Codex Home override inside a deliberate settings dialog", async () => {
    const setCodexHome = vi.fn().mockResolvedValue(undefined);
    vi.mocked(useMonitor).mockReturnValue({
      snapshot,
      settings: { codex_home_label: "~/.codex", is_default: true },
      loading: false,
      refreshing: false,
      error: null,
      connected: true,
      refresh: vi.fn(),
      setCodexHome,
    });
    render(<MonitorPage />);

    fireEvent.click(screen.getByRole("button", { name: "设置" }));
    expect(screen.getByRole("dialog", { name: "监控数据目录" })).toBeInTheDocument();
    expect(screen.getAllByText("~/.codex")).toHaveLength(2);
    fireEvent.change(screen.getByLabelText("自定义 Codex Home"), {
      target: { value: " D:\\CodexHome " },
    });
    fireEvent.click(screen.getByRole("button", { name: "验证并保存" }));

    await waitFor(() => expect(setCodexHome).toHaveBeenCalledWith("D:\\CodexHome"));
  });
});
