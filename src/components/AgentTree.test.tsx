import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { AgentObservation } from "../../shared/monitor-types";
import { AgentTree } from "./AgentTree";

const root: AgentObservation = {
  thread_id: "root",
  parent_thread_id: null,
  agent_path: null,
  display_name: "主任务",
  role: null,
  project: "monitor",
  originator: null,
  requested_model: "gpt-5.6-sol",
  effective_model: "gpt-5.6-sol",
  model_source: "turn-context",
  reasoning_effort: "high",
  status: "idle",
  model_drift: false,
  is_subagent: false,
  depth: 0,
  started_at_ms: 1,
  updated_at_ms: 2,
  freshness_ms: 10,
};

const child: AgentObservation = {
  ...root,
  thread_id: "child",
  parent_thread_id: "root",
  display_name: "后端实现",
  requested_model: "gpt-5.6-sol",
  effective_model: "gpt-5.6-terra",
  status: "running",
  model_drift: true,
  is_subagent: true,
  depth: 1,
};

describe("AgentTree", () => {
  it("keeps the parent visible when an active child matches", () => {
    render(
      <AgentTree
        agents={[root, child]}
        filters={{ query: "后端", model: "all", project: "all", activeOnly: true }}
      />,
    );
    expect(screen.getByText("主任务")).toBeInTheDocument();
    expect(screen.getByText("后端实现")).toBeInTheDocument();
    expect(screen.getByText("模型漂移")).toBeInTheDocument();
  });

  it("does not render idle-only matches in active mode", () => {
    render(
      <AgentTree
        agents={[root]}
        filters={{ query: "", model: "all", project: "all", activeOnly: true }}
      />,
    );
    expect(screen.getByText("没有匹配的代理")).toBeInTheDocument();
  });

  it("places a verified Smart Routing control only on each root row", () => {
    const setRootEnabled = vi.fn();
    render(
      <AgentTree
        agents={[root, child]}
        filters={{ query: "", model: "all", project: "all", activeOnly: false }}
        routing={{
          available: true,
          operationActive: false,
          routes: [{ conversation_id: "root", enabled: true }],
          controls: [{ conversation_id: "root", status: "pending-open" }],
          onSetRootEnabled: setRootEnabled,
        }}
      />,
    );

    expect(screen.getByText("等待打开")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "关闭 主任务 Smart Routing" })).toBeEnabled();
    expect(
      screen.queryByRole("button", { name: /后端实现 Smart Routing/ }),
    ).not.toBeInTheDocument();
    expect(screen.getByText(/数据不足/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "关闭 主任务 Smart Routing" }));
    expect(setRootEnabled).toHaveBeenCalledWith("root", false);
  });

  it("explains quality-first routing when a root enables it for the first time", () => {
    const setRootEnabled = vi.fn();
    render(
      <AgentTree
        agents={[root]}
        filters={{ query: "", model: "all", project: "all", activeOnly: false }}
        routing={{
          available: true,
          operationActive: false,
          routes: [{ conversation_id: "root", enabled: false }],
          controls: [{ conversation_id: "root", status: "off" }],
          onSetRootEnabled: setRootEnabled,
        }}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "启用 主任务 Smart Routing" }));

    expect(setRootEnabled).toHaveBeenCalledWith("root", true);
    expect(screen.getByRole("status")).toHaveTextContent("质量优先");
    expect(screen.getByRole("status")).toHaveTextContent("下一条消息");
  });
});
