import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useRouting } from "../hooks/useRouting";
import { SmartRoutingPage } from "./SmartRoutingPage";

vi.mock("../hooks/useRouting", () => ({ useRouting: vi.fn() }));

describe("SmartRoutingPage", () => {
  it("explains the uninstalled state before offering the owned setup", () => {
    vi.mocked(useRouting).mockReturnValue({
      snapshot: {
        contract_version: 2,
        setup: {
          installation_status: "uninstalled",
          restart_status: "not-required",
          preflight_status: "not-started",
          cdp_status: "inactive",
          backup_label: null,
          config_changes: [
            "agents.max_depth",
            "agents.codex_assistant_luna",
            "mcp_servers.codex_assistant_routing",
          ],
          reason_codes: [],
        },
        routing: {
          schema_version: 1,
          profile_version: "routing-v1",
          routes: [],
          eligibility: [],
          activity: [],
          quality: [],
        },
        controls: [],
      },
      loading: false,
      refreshing: false,
      degraded: false,
      error: null,
      connected: true,
      operation: null,
      receipt: null,
      pendingForce: null,
      refresh: vi.fn(),
      install: vi.fn(),
      restore: vi.fn(),
      requestRestart: vi.fn(),
      confirmForceRestart: vi.fn(),
      cancelForceRestart: vi.fn(),
      beginPreflight: vi.fn(),
      setRootEnabled: vi.fn(),
    });

    render(<SmartRoutingPage />);

    expect(screen.getByRole("heading", { name: "Smart Routing" })).toBeInTheDocument();
    expect(screen.getByText("尚未安装")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "安装 Smart Routing" })).toBeInTheDocument();
    expect(screen.getByText(/同一 Windows 用户/)).toBeInTheDocument();
    expect(screen.getByText(/不会读取或保存对话内容/)).toBeInTheDocument();
    expect(screen.queryByText(/节省.*%/)).not.toBeInTheDocument();
  });

  it("shows direct and nested native eligibility without simulating unavailable models", () => {
    const restore = vi.fn();
    const setRootEnabled = vi.fn();
    vi.mocked(useRouting).mockReturnValue({
      snapshot: {
        contract_version: 2,
        setup: {
          installation_status: "installed",
          restart_status: "not-required",
          preflight_status: "complete",
          cdp_status: "ready",
          backup_label: "routing-backup-20260718",
          config_changes: [],
          reason_codes: [],
        },
        routing: {
          schema_version: 1,
          profile_version: "routing-v1",
          routes: [
            {
              route_key: "d2719d93-b823-4a7f-934f-23cbe01c8aaf",
              conversation_id: "d2719d93-b823-4a7f-934f-23cbe01c8ab0",
              enabled: true,
              phase: "enabled",
              created_at_ms: 1,
              updated_at_ms: 2,
            },
          ],
          eligibility: [
            {
              tier: "luna",
              route_kind: "direct",
              status: "eligible",
              checked_at_ms: 1,
              profile_version: "routing-v1",
              codex_package_version: "26.715.3651.0",
              requested_model: "gpt-5.6-luna",
              depth: 1,
              reason: null,
            },
            {
              tier: "luna",
              route_kind: "nested",
              status: "unavailable",
              checked_at_ms: 1,
              profile_version: "routing-v1",
              codex_package_version: "26.715.3651.0",
              requested_model: "gpt-5.6-luna",
              depth: 2,
              reason: "parent-not-verified-terra",
            },
            {
              tier: "spark",
              route_kind: "direct",
              status: "unavailable",
              checked_at_ms: 1,
              profile_version: "routing-v1",
              codex_package_version: "26.715.3651.0",
              requested_model: "gpt-5.3-codex-spark",
              depth: 1,
              reason: "native-profile-rejected",
            },
          ],
          activity: [],
          quality: [],
        },
        controls: [
          {
            conversation_id: "d2719d93-b823-4a7f-934f-23cbe01c8ab0",
            status: "enabled",
          },
        ],
      },
      loading: false,
      refreshing: false,
      degraded: false,
      error: null,
      connected: true,
      operation: null,
      receipt: null,
      pendingForce: null,
      refresh: vi.fn(),
      install: vi.fn(),
      restore,
      requestRestart: vi.fn(),
      confirmForceRestart: vi.fn(),
      cancelForceRestart: vi.fn(),
      beginPreflight: vi.fn(),
      setRootEnabled,
    });

    render(
      <SmartRoutingPage
        roots={[
          {
            conversationId: "d2719d93-b823-4a7f-934f-23cbe01c8ab0",
            label: "当前任务",
          },
        ]}
      />,
    );

    expect(screen.getByRole("row", { name: /Luna.*直接.*已验证/ })).toHaveTextContent(
      "gpt-5.6-luna",
    );
    expect(screen.getByRole("row", { name: /Luna.*嵌套.*不可用/ })).toHaveTextContent(
      "需要已验证的 Terra 父代理",
    );
    expect(screen.getByRole("row", { name: /Spark.*直接.*不可用/ })).toHaveTextContent(
      "原生配置被 Codex 拒绝",
    );
    expect(screen.getAllByText("26.715.3651.0")).toHaveLength(3);
    expect(screen.getByText(/机械任务.*Spark/)).toBeInTheDocument();
    expect(screen.getByText(/低风险边界任务.*Luna/)).toBeInTheDocument();
    expect(screen.getByText(/最多 3 个活跃子代理/)).toBeInTheDocument();
    expect(screen.getByText(/每次实现必须经过独立复审/)).toBeInTheDocument();
    expect(screen.getByText("routing-backup-20260718")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "恢复官方配置" }));
    expect(screen.getByRole("dialog", { name: "恢复官方配置" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "确认恢复" }));
    expect(restore).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "关闭 当前任务 Smart Routing" }));
    expect(setRootEnabled).toHaveBeenCalledWith("d2719d93-b823-4a7f-934f-23cbe01c8ab0", false);
  });

  it("offers the verified one-time restart only when configuration requires it", () => {
    const requestRestart = vi.fn();
    vi.mocked(useRouting).mockReturnValue({
      snapshot: {
        contract_version: 2,
        setup: {
          installation_status: "restart-required",
          restart_status: "required",
          preflight_status: "not-started",
          cdp_status: "inactive",
          backup_label: "routing-backup-20260718",
          config_changes: [],
          reason_codes: [],
        },
        routing: {
          schema_version: 1,
          profile_version: "routing-v1",
          routes: [],
          eligibility: [],
          activity: [],
          quality: [],
        },
        controls: [],
      },
      loading: false,
      refreshing: false,
      degraded: false,
      error: null,
      connected: true,
      operation: null,
      receipt: null,
      pendingForce: null,
      refresh: vi.fn(),
      install: vi.fn(),
      restore: vi.fn(),
      requestRestart,
      confirmForceRestart: vi.fn(),
      cancelForceRestart: vi.fn(),
      beginPreflight: vi.fn(),
      setRootEnabled: vi.fn(),
    });
    render(<SmartRoutingPage />);

    fireEvent.click(screen.getByRole("button", { name: "安全重启 Codex 一次" }));

    expect(requestRestart).toHaveBeenCalledOnce();
    expect(screen.getByText(/仅在没有运行中的原生子代理时/)).toBeInTheDocument();
  });

  it("keeps the blocked restart actionable and shows the force confirmation impact", () => {
    const requestRestart = vi.fn();
    const confirmForceRestart = vi.fn();
    vi.mocked(useRouting).mockReturnValue({
      snapshot: {
        contract_version: 2,
        setup: {
          installation_status: "restart-required",
          restart_status: "blocked-active-child",
          preflight_status: "not-started",
          cdp_status: "inactive",
          backup_label: "routing-backup-20260718",
          config_changes: [],
          reason_codes: ["active-child"],
        },
        routing: {
          schema_version: 1,
          profile_version: "routing-v1",
          routes: [],
          eligibility: [],
          activity: [],
          quality: [],
        },
        controls: [],
      },
      loading: false,
      refreshing: false,
      degraded: false,
      error: null,
      connected: true,
      operation: null,
      receipt: null,
      pendingForce: {
        confirmation_ticket: "d2719d93-b823-4a7f-934f-23cbe01c8ab0",
        intent: "routing-restart",
        active_native_children: 1,
        grace_period_ms: 5000,
        expires_at_ms: 100_000,
      },
      refresh: vi.fn(),
      install: vi.fn(),
      restore: vi.fn(),
      requestRestart,
      confirmForceRestart,
      cancelForceRestart: vi.fn(),
      beginPreflight: vi.fn(),
      setRootEnabled: vi.fn(),
    });
    render(<SmartRoutingPage />);

    const trigger = screen.getByRole("button", { name: "查看强制重启选项" });
    expect(trigger).toBeEnabled();
    fireEvent.click(trigger);
    expect(requestRestart).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "终止子代理并强制重启" }));
    expect(confirmForceRestart).toHaveBeenCalledOnce();
  });

  it("starts native preflight for the selected visible root task", () => {
    const beginPreflight = vi.fn();
    vi.mocked(useRouting).mockReturnValue({
      snapshot: {
        contract_version: 2,
        setup: {
          installation_status: "installed",
          restart_status: "not-required",
          preflight_status: "not-started",
          cdp_status: "ready",
          backup_label: "routing-backup-20260718",
          config_changes: [],
          reason_codes: [],
        },
        routing: {
          schema_version: 1,
          profile_version: "routing-v1",
          routes: [],
          eligibility: [],
          activity: [],
          quality: [],
        },
        controls: [],
      },
      loading: false,
      refreshing: false,
      degraded: false,
      error: null,
      connected: true,
      operation: null,
      receipt: null,
      pendingForce: null,
      refresh: vi.fn(),
      install: vi.fn(),
      restore: vi.fn(),
      requestRestart: vi.fn(),
      confirmForceRestart: vi.fn(),
      cancelForceRestart: vi.fn(),
      beginPreflight,
      setRootEnabled: vi.fn(),
    });
    render(
      <SmartRoutingPage
        roots={[
          {
            conversationId: "d2719d93-b823-4a7f-934f-23cbe01c8ab0",
            label: "当前任务",
          },
        ]}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "开始原生能力预检" }));

    expect(beginPreflight).toHaveBeenCalledWith("d2719d93-b823-4a7f-934f-23cbe01c8ab0");
    expect(screen.getByText(/会在当前任务的原生子代理面板中验证/)).toBeInTheDocument();
  });
});
