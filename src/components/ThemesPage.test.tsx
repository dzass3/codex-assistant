import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ThemePack, ThemeUiSnapshot } from "../../shared/theme-types";
import { useTheme } from "../hooks/useTheme";
import { ThemesPage } from "./ThemesPage";

vi.mock("../hooks/useTheme", () => ({ useTheme: vi.fn() }));

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
  backdrop: { kind: "gradient", angle: 135, colors: ["#07111f", "#18204b", "#0b4d5f"] },
  palette: {
    surface: "#101827",
    surface_strong: "#111b2d",
    text: "#eef7ff",
    accent: "#64e7ff",
    border: "#6fdcf0",
  },
  effects: { surface_opacity: 78, blur_px: 22, contrast_percent: 108, motion: true },
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

function state(snapshot: ThemeUiSnapshot, overrides: Record<string, unknown> = {}) {
  return {
    snapshot,
    loading: false,
    refreshing: false,
    degraded: false,
    error: null,
    connected: true,
    operation: null,
    receipt: null,
    pendingForce: null,
    refresh: vi.fn(),
    startSession: vi.fn(),
    activate: vi.fn(),
    restore: vi.fn(),
    confirmForceRestart: vi.fn(),
    cancelForceRestart: vi.fn(),
    ...overrides,
  };
}

describe("ThemesPage", () => {
  beforeEach(() => vi.mocked(useTheme).mockReset());

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

    expect(screen.getByRole("heading", { name: "主题管理" })).toBeInTheDocument();
    expect(screen.getByText(/安全重启一次 Codex/)).toBeInTheDocument();
    expect(screen.getByText(/运行中的原生子代理/)).toBeInTheDocument();
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
          active_native_children: 2,
          grace_period_ms: 5000,
          expires_at_ms: 100_000,
        },
        confirmForceRestart,
        cancelForceRestart,
      },
    );
    vi.mocked(useTheme).mockReturnValue(value);
    render(<ThemesPage />);

    const dialog = screen.getByRole("alertdialog", { name: "终止子代理并强制重启？" });
    expect(dialog).toHaveTextContent("当前有 2 个原生子代理");
    expect(dialog).toHaveTextContent("等待 5 秒");
    fireEvent.click(screen.getByRole("button", { name: "终止子代理并强制重启" }));
    expect(confirmForceRestart).toHaveBeenCalledOnce();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(cancelForceRestart).toHaveBeenCalledOnce();
  });
});
