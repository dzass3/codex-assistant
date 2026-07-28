import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import bundledCatalogJson from "../../shared/theme-catalog.json";
import type { ThemeEnvironmentReport, ThemePack, ThemeUiSnapshot } from "../../shared/theme-types";
import { useTheme } from "../hooks/useTheme";
import { themeApi } from "../lib/themeApi";
import { ThemesPage } from "./ThemesPage";

vi.mock("../hooks/useTheme", () => ({ useTheme: vi.fn() }));
vi.mock("../lib/themeApi", () => ({
  themeApi: { getPreviewDataUrl: vi.fn() },
}));

const rights = {
  source: "Original theme authored for Codex Assistant",
  rightsholder: "Codex Assistant project",
  license: "Project-owned distribution asset",
  commercial_redistribution: true,
  attribution: "Original artwork created for Codex Assistant",
  reviewed_at: "2026-07-18",
  manual_signoff: true,
  status: "verified" as const,
};

const aurora: ThemePack = {
  schema_version: 1,
  minimum_engine_version: 1,
  id: "aurora-grid",
  name: "Aurora Grid",
  description: "Project-owned abstract aurora.",
  category: "abstract",
  preview_path: "/themes/aurora-grid.svg",
  backdrop: {
    kind: "gradient",
    angle: 135,
    colors: ["#07111f", "#18204b", "#0b4d5f"],
  },
  palette: {
    surface: "#101827",
    surface_strong: "#111b2d",
    text: "#eef7ff",
    accent: "#64e7ff",
    border: "#6fdcf0",
  },
  effects: {
    surface_opacity: 78,
    blur_px: 22,
    contrast_percent: 108,
    motion: true,
  },
  assets: [],
  rights,
};

const muse: ThemePack = {
  ...aurora,
  id: "observatory-muse",
  name: "Observatory Muse",
  description: "Original fictional technologist.",
  category: "original-character",
  preview_path: "/themes/original-observatory-muse.jpg",
  backdrop: {
    kind: "image",
    asset_id: "original-observatory-muse",
    overlay: "#071326",
    focal_x: 50,
    focal_y: 50,
  },
  assets: [
    {
      id: "original-observatory-muse",
      mime_type: "image/jpeg",
      sha256: "a486add3d4c8efb5c8f9b28c453291c0ed2192d415565f0bec1dc6f9bdc78c3a",
    },
  ],
};

const bundledCatalog = bundledCatalogJson as unknown as { themes: ThemePack[] };
const wisteriaBride = bundledCatalog.themes.find(
  (pack) => pack.id === "wisteria-bride",
) as ThemePack;

const environment: ThemeEnvironmentReport = {
  contract_version: 2,
  status: "ready",
  checks: [
    { code: "supported-windows", state: "pass" },
    { code: "supported-architecture", state: "pass" },
    { code: "official-store-codex", state: "pass" },
    { code: "compatible-adapter", state: "pass" },
    { code: "single-codex-window", state: "pass" },
    { code: "verified-theme-session", state: "pass" },
    { code: "saved-theme", state: "action" },
  ],
  os_build: 22621,
  architecture: "x64",
  codex_version: "26.715.8383.0",
  verified_process_count: 1,
  session_reachable: true,
  selected_theme_id: null,
  next_action: "apply-now",
  can_apply_now: true,
};

function state(snapshot: ThemeUiSnapshot, overrides: Record<string, unknown> = {}) {
  return {
    snapshot,
    environment,
    loading: false,
    refreshing: false,
    degraded: false,
    error: null,
    connected: true,
    operation: null,
    receipt: null,
    pendingForce: null,
    refresh: vi.fn(),
    refreshEnvironment: vi.fn(),
    startSession: vi.fn(),
    activate: vi.fn(),
    restore: vi.fn(),
    confirmForceRestart: vi.fn(),
    cancelForceRestart: vi.fn(),
    importLocalImage: vi.fn(),
    ...overrides,
  };
}

describe("ThemesPage", () => {
  beforeEach(() => {
    vi.mocked(useTheme).mockReset();
    vi.mocked(themeApi.getPreviewDataUrl).mockReset();
  });

  it("starts the verified session and applies a selected theme with one click", () => {
    const value = state({
      contract_version: 2,
      session_status: "inactive",
      selected_theme_id: null,
      applied_theme_id: null,
      packs: [aurora, muse],
    });
    vi.mocked(useTheme).mockReturnValue(value);
    render(<ThemesPage />);

    expect(screen.getByRole("heading", { name: "一键换肤" })).toBeInTheDocument();
    expect(screen.getByText(/首次应用可能需要你确认重启官方 ChatGPT\/Codex/)).toBeInTheDocument();
    const applyButtons = screen.getAllByRole("button", { name: "应用主题" });
    expect(applyButtons[0]).toBeEnabled();
    fireEvent.click(applyButtons[0]);
    expect(value.activate).toHaveBeenCalledWith("aurora-grid");

    fireEvent.click(screen.getByRole("button", { name: "启动主题会话" }));
    expect(value.startSession).toHaveBeenCalledOnce();
  });

  it("shows only rights-verified bundled themes and applies one by its fixed identifier", () => {
    const value = state({
      contract_version: 2,
      session_status: "ready",
      selected_theme_id: null,
      applied_theme_id: null,
      packs: [aurora, muse],
    });
    vi.mocked(useTheme).mockReturnValue(value);
    render(<ThemesPage />);

    expect(screen.getAllByText("版权已核验")).toHaveLength(2);
    expect(screen.getByAltText("Observatory Muse 主题预览")).toHaveAttribute(
      "src",
      "/themes/original-observatory-muse.jpg",
    );
    fireEvent.click(screen.getAllByRole("button", { name: "应用主题" })[1]);
    expect(value.activate).toHaveBeenCalledWith("observatory-muse");
    expect(screen.getByText(/名人、动漫\/IP 和第三方仓库截图不会随应用分发/)).toBeInTheDocument();
  });

  it("shows the real offline Wisteria Bride card and activates its stable identifier", () => {
    const value = state({
      contract_version: 2,
      session_status: "ready",
      selected_theme_id: null,
      applied_theme_id: null,
      packs: [wisteriaBride],
    });
    vi.mocked(useTheme).mockReturnValue(value);

    render(<ThemesPage />);

    expect(screen.getByRole("heading", { name: "紫藤花嫁" })).toBeVisible();
    expect(screen.getByAltText("紫藤花嫁 主题预览")).toHaveAttribute(
      "src",
      "/themes/wisteria-bride.webp",
    );
    const preview = screen.getByAltText("紫藤花嫁 主题预览").closest(".theme-card__preview");
    expect(preview?.querySelectorAll("img")).toHaveLength(2);
    expect(screen.getByAltText("紫藤花嫁 主题预览")).toHaveClass("theme-card__preview-artwork");
    expect(preview?.querySelector(".theme-card__preview-backdrop")).toHaveAttribute(
      "aria-hidden",
      "true",
    );
    expect(screen.getByText("Codex Assistant asset contributor")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "应用主题" }));
    expect(value.activate).toHaveBeenCalledWith("wisteria-bride");
  });

  it("explains a retired bundled preference without hiding the replacement catalog", () => {
    const value = state({
      contract_version: 2,
      session_status: "inactive",
      selected_theme_id: null,
      applied_theme_id: null,
      catalog_notice: "原主题已下架，请从 14 个新主题中重新选择",
      packs: [wisteriaBride],
    });
    vi.mocked(useTheme).mockReturnValue(value);

    render(<ThemesPage />);

    expect(screen.getByText("原主题已下架，请从 14 个新主题中重新选择")).toHaveAttribute(
      "role",
      "status",
    );
    expect(screen.getByRole("heading", { name: "紫藤花嫁" })).toBeVisible();
  });

  it("renders a real local-only preview and never labels it rights-verified", async () => {
    const local: ThemePack = {
      ...muse,
      id: "arina-pink",
      name: "Arina 粉晶花园",
      category: "local-import",
      preview_path: "local-theme:arina-pink",
      backdrop: {
        kind: "image",
        asset_id: "arina-pink",
        overlay: "#fff5f6",
        focal_x: 72,
        focal_y: 45,
      },
      assets: [{ ...muse.assets[0], id: "arina-pink" }],
      rights: {
        ...rights,
        commercial_redistribution: false,
        status: "local-only",
      },
    };
    vi.mocked(themeApi.getPreviewDataUrl).mockResolvedValue("data:image/jpeg;base64,YQ==");
    vi.mocked(useTheme).mockReturnValue(
      state({
        contract_version: 2,
        session_status: "ready",
        selected_theme_id: null,
        applied_theme_id: null,
        packs: [local],
      }),
    );

    render(<ThemesPage />);

    expect(await screen.findByAltText("Arina 粉晶花园 主题预览")).toHaveAttribute(
      "src",
      "data:image/jpeg;base64,YQ==",
    );
    expect(screen.getByText("仅限本机")).toBeVisible();
    expect(screen.queryByText("版权已核验")).not.toBeInTheDocument();
  });

  it("imports a selected local image through the visible import control", async () => {
    const importLocalImage = vi.fn().mockResolvedValue(null);
    vi.mocked(useTheme).mockReturnValue(
      state(
        {
          contract_version: 2,
          session_status: "ready",
          selected_theme_id: null,
          applied_theme_id: null,
          packs: [aurora],
        },
        { importLocalImage },
      ),
    );
    const { container } = render(<ThemesPage />);
    const input = container.querySelector<HTMLInputElement>('input[type="file"]');
    const file = new File([new Uint8Array([0x52, 0x49, 0x46, 0x46])], "my-garden.webp", {
      type: "image/webp",
    });

    expect(screen.getByRole("button", { name: "导入本机图片" })).toBeEnabled();
    fireEvent.change(input!, { target: { files: [file] } });

    await vi.waitFor(() =>
      expect(importLocalImage).toHaveBeenCalledWith(
        "my-garden",
        expect.stringMatching(/^data:image\/webp;base64,/),
      ),
    );
  });

  it("marks the active theme and restores the official Codex appearance", () => {
    const value = state({
      contract_version: 2,
      session_status: "ready",
      selected_theme_id: "aurora-grid",
      applied_theme_id: "aurora-grid",
      packs: [aurora, muse],
    });
    vi.mocked(useTheme).mockReturnValue(value);
    render(<ThemesPage />);

    expect(screen.getByRole("button", { name: "当前主题" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "恢复官方外观" }));
    expect(value.restore).toHaveBeenCalledOnce();
  });

  it("requires an accessible destructive confirmation before force restarting", () => {
    const confirmForceRestart = vi.fn();
    const cancelForceRestart = vi.fn();
    const value = state(
      {
        contract_version: 2,
        session_status: "inactive",
        selected_theme_id: "aurora-grid",
        applied_theme_id: null,
        packs: [aurora],
      },
      {
        pendingForce: {
          confirmation_ticket: "d2719d93-b823-4a7f-934f-23cbe01c8ab0",
          intent: "activate-theme",
          active_work_count: 2,
          monitor_confident: true,
          grace_period_ms: 5000,
          expires_at_ms: 100_000,
        },
        confirmForceRestart,
        cancelForceRestart,
      },
    );
    vi.mocked(useTheme).mockReturnValue(value);
    render(<ThemesPage />);

    const dialog = screen.getByRole("alertdialog", {
      name: "结束运行中的任务并重启？",
    });
    expect(dialog).toHaveTextContent("当前有 2 个 Codex 任务");
    expect(dialog).toHaveTextContent("等待 5 秒");
    fireEvent.click(screen.getByRole("button", { name: "结束任务并强制重启" }));
    expect(confirmForceRestart).toHaveBeenCalledOnce();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(cancelForceRestart).toHaveBeenCalledOnce();
  });

  it("restores focus to the control that triggered the restart check", () => {
    const base = state({
      contract_version: 2,
      session_status: "inactive",
      selected_theme_id: null,
      applied_theme_id: null,
      packs: [aurora],
    });
    vi.mocked(useTheme).mockReturnValue(base);
    const view = render(<ThemesPage />);
    const trigger = screen.getByRole("button", { name: "应用主题" });

    trigger.focus();
    fireEvent.click(trigger);
    expect(base.activate).toHaveBeenCalledWith("aurora-grid");

    const cancelForceRestart = vi.fn();
    vi.mocked(useTheme).mockReturnValue({
      ...base,
      pendingForce: {
        confirmation_ticket: "d2719d93-b823-4a7f-934f-23cbe01c8ab0",
        intent: "activate-theme",
        active_work_count: 1,
        monitor_confident: true,
        grace_period_ms: 5000,
        expires_at_ms: 100_000,
      },
      cancelForceRestart,
    });
    view.rerender(<ThemesPage />);

    expect(screen.getByRole("button", { name: "取消" })).toHaveFocus();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(cancelForceRestart).toHaveBeenCalledOnce();

    vi.mocked(useTheme).mockReturnValue(base);
    view.rerender(<ThemesPage />);
    expect(trigger).toHaveFocus();
  });

  it("shows a paused preference with explicit resume and official-appearance actions", () => {
    const value = state({
      contract_version: 2,
      session_status: "paused",
      selected_theme_id: "aurora-grid",
      applied_theme_id: null,
      packs: [aurora],
    });
    vi.mocked(useTheme).mockReturnValue(value);
    render(<ThemesPage />);

    expect(screen.getByRole("heading", { name: "主题已暂停" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "恢复主题会话" }));
    expect(value.activate).toHaveBeenCalledWith("aurora-grid");

    fireEvent.click(screen.getByRole("button", { name: "取消主题并保持官方外观" }));
    expect(value.restore).toHaveBeenCalledOnce();
  });

  it("shows exact local readiness and a guarded restart action for an ordinary Codex launch", () => {
    const value = state(
      {
        contract_version: 2,
        session_status: "paused",
        selected_theme_id: "aurora-grid",
        applied_theme_id: null,
        packs: [aurora],
      },
      {
        environment: {
          ...environment,
          status: "restart-required",
          session_reachable: false,
          selected_theme_id: "aurora-grid",
          next_action: "confirm-restart",
          can_apply_now: false,
          checks: environment.checks.map((check) =>
            check.code === "verified-theme-session"
              ? { ...check, state: "action" as const }
              : check,
          ),
        },
      },
    );
    vi.mocked(useTheme).mockReturnValue(value);
    render(<ThemesPage />);

    expect(screen.getByRole("heading", { name: "本机环境检测" })).toBeInTheDocument();
    expect(screen.getByText(/Codex 26\.715\.8383\.0/)).toBeVisible();
    expect(screen.getByText(/当前官方应用无法在运行后补加/)).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "确认重启并应用" }));
    expect(value.activate).toHaveBeenCalledWith("aurora-grid");
  });

  it("guides clean and ambiguous machines without claiming theme readiness", () => {
    vi.mocked(useTheme).mockReturnValue(
      state(
        {
          contract_version: 2,
          session_status: "inactive",
          selected_theme_id: null,
          applied_theme_id: null,
          packs: [aurora],
        },
        {
          environment: {
            ...environment,
            status: "unsupported",
            codex_version: null,
            verified_process_count: 0,
            session_reachable: false,
            next_action: "install-codex",
            can_apply_now: false,
          },
        },
      ),
    );
    const { rerender } = render(<ThemesPage />);
    expect(screen.getByText(/Microsoft Store 安装官方 Codex/)).toBeVisible();

    vi.mocked(useTheme).mockReturnValue(
      state(
        {
          contract_version: 2,
          session_status: "inactive",
          selected_theme_id: null,
          applied_theme_id: null,
          packs: [aurora],
        },
        {
          environment: {
            ...environment,
            status: "unsupported",
            verified_process_count: 2,
            session_reachable: false,
            next_action: "close-extra-windows",
            can_apply_now: false,
          },
        },
      ),
    );
    rerender(<ThemesPage />);
    expect(screen.getByText(/关闭多余的 Codex 窗口/)).toBeVisible();
  });

  it("states that a normal full reopen requires another explicit apply", () => {
    vi.mocked(useTheme).mockReturnValue(
      state({
        contract_version: 2,
        session_status: "ready",
        selected_theme_id: "aurora-grid",
        applied_theme_id: "aurora-grid",
        packs: [aurora],
      }),
    );
    render(<ThemesPage />);
    expect(
      screen.getByText(
        "主题选择会保留；完全关闭并从官方入口重新打开 ChatGPT/Codex 后，需要回到这里再次点击“应用主题”。",
      ),
    ).toBeVisible();
    expect(screen.queryByText(/Codex（主题版）/)).not.toBeInTheDocument();
  });

  it("labels an unmanaged running Codex as an explicit restart", () => {
    vi.mocked(useTheme).mockReturnValue(
      state(
        {
          contract_version: 2,
          session_status: "inactive",
          selected_theme_id: null,
          applied_theme_id: null,
          packs: [aurora],
        },
        {
          environment: {
            ...environment,
            status: "restart-required",
            session_reachable: false,
            next_action: "confirm-restart",
            can_apply_now: false,
          },
        },
      ),
    );
    render(<ThemesPage />);
    expect(screen.getByRole("button", { name: "确认重启并应用" })).toBeEnabled();
  });

  it("labels a stopped official app as a one-shot user launch", () => {
    vi.mocked(useTheme).mockReturnValue(
      state(
        {
          contract_version: 2,
          session_status: "inactive",
          selected_theme_id: "aurora-grid",
          applied_theme_id: null,
          packs: [aurora],
        },
        {
          environment: {
            ...environment,
            status: "codex-not-running",
            verified_process_count: 0,
            session_reachable: false,
            next_action: "launch-codex-for-theme",
            can_apply_now: false,
          },
        },
      ),
    );
    render(<ThemesPage />);
    expect(screen.getByRole("button", { name: "启动并应用主题" })).toBeEnabled();
  });
});
