import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { AgentObservation } from "../../shared/monitor-types";
import { AgentTree } from "./AgentTree";

const root: AgentObservation = {
  thread_id: "root",
  parent_thread_id: null,
  display_name: "主任务",
  role: null,
  project: "monitor",
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
  updated_at_ms: 2,
  freshness_ms: 10,
};

const child: AgentObservation = {
  ...root,
  thread_id: "child",
  parent_thread_id: "root",
  display_name: "后端实现",
  role: "worker",
  requested_model: "gpt-5.6-sol",
  effective_model: "gpt-5.6-terra",
  reasoning_effort: "xhigh",
  status: "running",
  model_drift: true,
  is_subagent: true,
  depth: 1,
};

describe("AgentTree", () => {
  it("keeps an idle parent visible when an active child matches", () => {
    render(
      <AgentTree
        agents={[root, child]}
        filters={{ query: "后端", model: "all", project: "all", activeOnly: true }}
      />,
    );

    expect(screen.getByText("主任务")).toBeInTheDocument();
    expect(screen.getByText("后端实现")).toBeInTheDocument();
    expect(screen.getByText("gpt-5.6-terra")).toBeInTheDocument();
    expect(screen.getByText("xhigh")).toBeInTheDocument();
    expect(screen.getByText("模型漂移")).toBeInTheDocument();
    expect(screen.queryByText(/Smart Routing/i)).not.toBeInTheDocument();
  });

  it("labels requested-only model intent without presenting it as effective", () => {
    render(
      <AgentTree
        agents={[
          {
            ...child,
            thread_id: "pending",
            display_name: "待启动审查",
            requested_model: "gpt-5.6-terra",
            effective_model: null,
            model_source: "requested-only",
            status: "starting",
            model_drift: false,
          },
        ]}
        filters={{ query: "", model: "all", project: "all", activeOnly: true }}
      />,
    );

    expect(screen.getByText("尚未确认")).toBeInTheDocument();
    expect(screen.getByText("仅请求值")).toBeInTheDocument();
    expect(screen.getByText("请求 gpt-5.6-terra")).toBeInTheDocument();
  });

  it("reveals idle and interrupted agents only in all mode", () => {
    const ended = {
      ...child,
      thread_id: "ended",
      parent_thread_id: null,
      display_name: "已中断审查",
      status: "interrupted" as const,
    };
    const { rerender } = render(
      <AgentTree
        agents={[root, ended]}
        filters={{ query: "", model: "all", project: "all", activeOnly: true }}
      />,
    );

    expect(screen.queryByText("已中断审查")).not.toBeInTheDocument();

    rerender(
      <AgentTree
        agents={[root, ended]}
        filters={{ query: "", model: "all", project: "all", activeOnly: false }}
      />,
    );
    expect(screen.getByText("主任务")).toBeInTheDocument();
    expect(screen.getByText("已中断审查")).toBeInTheDocument();
  });

  it("labels stale unclosed work as history and formats ages over one day", () => {
    render(
      <AgentTree
        agents={[
          {
            ...child,
            status: "historical-unclosed",
            freshness_ms: 211 * 60 * 60 * 1_000,
            updated_at_ms: Date.UTC(2026, 6, 18, 8, 0, 0),
          },
        ]}
        filters={{ query: "", model: "all", project: "all", activeOnly: false }}
      />,
    );
    expect(screen.getByText("历史状态未闭合")).toBeInTheDocument();
    expect(screen.getByText(/8 天前/)).toBeInTheDocument();
    expect(screen.getByTestId("agent-child")).toHaveAttribute("title");
  });

  it("does not infer activity when the official app is stopped", () => {
    render(
      <AgentTree
        agents={[]}
        codexRunning={false}
        filters={{ query: "", model: "all", project: "all", activeOnly: true }}
      />,
    );
    expect(screen.getByRole("heading", { name: "Codex 未运行" })).toBeInTheDocument();
  });
});
