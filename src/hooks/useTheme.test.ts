import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type {
  ThemeEnvironmentReport,
  ThemeOperationReceipt,
  ThemeUiSnapshot,
} from "../../shared/theme-types";
import { themeApi } from "../lib/themeApi";
import { useTheme } from "./useTheme";

vi.mock("../lib/themeApi", () => ({
  themeApi: {
    getSnapshot: vi.fn(),
    getEnvironment: vi.fn(),
    startSession: vi.fn(),
    prepareForceRestart: vi.fn(),
    importLocalImage: vi.fn(),
    activate: vi.fn(),
    restore: vi.fn(),
  },
}));

const validSnapshot: ThemeUiSnapshot = {
  contract_version: 2,
  session_status: "ready",
  selected_theme_id: null,
  applied_theme_id: null,
  packs: [],
};

const validEnvironment: ThemeEnvironmentReport = {
  contract_version: 2,
  status: "ready",
  checks: [],
  os_build: 22621,
  architecture: "x64",
  codex_version: "26.715.8383.0",
  verified_process_count: 1,
  session_reachable: true,
  selected_theme_id: null,
  next_action: "apply-now",
  can_apply_now: true,
};

const receipt: ThemeOperationReceipt = {
  operation_id: "d2719d93-b823-4a7f-934f-23cbe01c8ab0",
  status: "applied",
  reason_codes: [],
  restart_required: false,
};

describe("useTheme", () => {
  afterEach(() => {
    vi.clearAllTimers();
    vi.useRealTimers();
  });

  beforeEach(() => {
    vi.mocked(themeApi.getSnapshot).mockReset();
    vi.mocked(themeApi.getEnvironment).mockReset();
    vi.mocked(themeApi.getEnvironment).mockResolvedValue(validEnvironment);
    vi.mocked(themeApi.startSession).mockReset();
    vi.mocked(themeApi.prepareForceRestart).mockReset();
    vi.mocked(themeApi.importLocalImage).mockReset();
    vi.mocked(themeApi.activate).mockReset();
    vi.mocked(themeApi.restore).mockReset();
  });

  it("retains the last verified theme snapshot when refresh fails closed", async () => {
    vi.mocked(themeApi.getSnapshot)
      .mockResolvedValueOnce(validSnapshot)
      .mockResolvedValueOnce(null);
    const { result } = renderHook(() => useTheme());
    await waitFor(() => expect(result.current.snapshot).toEqual(validSnapshot));

    await act(() => result.current.refresh());

    expect(result.current.snapshot).toEqual(validSnapshot);
    expect(result.current.degraded).toBe(true);
    expect(result.current.error).toMatch(/上一次/);
  });

  it("polls environment state without mutating a theme", async () => {
    vi.useFakeTimers();
    vi.mocked(themeApi.getSnapshot).mockResolvedValue(validSnapshot);
    const { result, unmount } = renderHook(() => useTheme());
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(result.current.snapshot).toEqual(validSnapshot);
    vi.mocked(themeApi.startSession).mockClear();
    vi.mocked(themeApi.activate).mockClear();
    vi.mocked(themeApi.prepareForceRestart).mockClear();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(5_000);
    });

    expect(themeApi.getEnvironment).toHaveBeenCalledTimes(2);
    expect(themeApi.startSession).not.toHaveBeenCalled();
    expect(themeApi.activate).not.toHaveBeenCalled();
    expect(themeApi.prepareForceRestart).not.toHaveBeenCalled();
    unmount();
    vi.useRealTimers();
  });

  it("requires a fresh force confirmation when monitor health is uncertain", async () => {
    vi.mocked(themeApi.getSnapshot).mockResolvedValue(validSnapshot);
    vi.mocked(themeApi.startSession).mockResolvedValue({
      operation_id: "d2719d93-b823-4a7f-934f-23cbe01c8ab2",
      status: "blocked",
      reason_codes: ["monitor-uncertain"],
      restart_required: false,
    });
    vi.mocked(themeApi.prepareForceRestart).mockResolvedValue({
      confirmation_ticket: "d2719d93-b823-4a7f-934f-23cbe01c8ab3",
      intent: "theme-session",
      active_work_count: 0,
      monitor_confident: false,
      grace_period_ms: 5000,
      expires_at_ms: 100_000,
    });
    const { result } = renderHook(() => useTheme());
    await waitFor(() => expect(result.current.snapshot).toEqual(validSnapshot));

    await act(() => result.current.startSession());

    expect(themeApi.prepareForceRestart).toHaveBeenCalledWith("theme-session", undefined);
    expect(result.current.pendingForce).toMatchObject({
      active_work_count: 0,
      monitor_confident: false,
    });
    expect(result.current.error).toBeNull();
  });

  it("preserves the actionable monitor failure when restart impact cannot be prepared", async () => {
    vi.mocked(themeApi.getSnapshot).mockResolvedValue(validSnapshot);
    vi.mocked(themeApi.activate).mockResolvedValue({
      operation_id: "d2719d93-b823-4a7f-934f-23cbe01c8ab4",
      status: "blocked",
      reason_codes: ["monitor-uncertain"],
      restart_required: false,
    });
    vi.mocked(themeApi.prepareForceRestart).mockRejectedValue(new Error("CdpUnavailable"));
    const { result } = renderHook(() => useTheme());
    await waitFor(() => expect(result.current.snapshot).toEqual(validSnapshot));

    await act(() => result.current.activate("wisteria-bride"));

    expect(result.current.receipt).toMatchObject({
      status: "blocked",
      reason_codes: ["monitor-uncertain"],
    });
    expect(result.current.error).toMatch(/监控数据不完整|普通重启已阻止/);
    expect(result.current.error).not.toMatch(/主题操作失败/);
  });

  it("serializes session and apply mutations then refreshes confirmed state", async () => {
    let finishStart: ((value: ThemeOperationReceipt) => void) | undefined;
    vi.mocked(themeApi.getSnapshot).mockResolvedValue(validSnapshot);
    vi.mocked(themeApi.startSession).mockReturnValue(
      new Promise<ThemeOperationReceipt>((resolve) => {
        finishStart = resolve;
      }),
    );
    vi.mocked(themeApi.activate).mockResolvedValue(receipt);
    const { result } = renderHook(() => useTheme());
    await waitFor(() => expect(result.current.snapshot).toEqual(validSnapshot));

    let starting: Promise<unknown> | undefined;
    act(() => {
      starting = result.current.startSession();
    });
    expect(result.current.operation).toBe("start-session");
    await expect(result.current.activate("aurora-grid")).resolves.toBeNull();
    expect(themeApi.activate).not.toHaveBeenCalled();

    await act(async () => {
      finishStart?.(receipt);
      await starting;
    });
    await act(() => result.current.activate("aurora-grid"));

    expect(themeApi.startSession).toHaveBeenCalledOnce();
    expect(themeApi.activate).toHaveBeenCalledWith("aurora-grid");
    expect(themeApi.getSnapshot).toHaveBeenCalledTimes(3);
    expect(result.current.operation).toBeNull();
  });

  it("releases a stalled activation instead of leaving every theme card busy forever", async () => {
    vi.useFakeTimers();
    vi.mocked(themeApi.getSnapshot).mockResolvedValue(validSnapshot);
    vi.mocked(themeApi.activate).mockReturnValue(new Promise(() => undefined));
    const { result, unmount } = renderHook(() => useTheme());
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(result.current.snapshot).toEqual(validSnapshot);

    act(() => {
      void result.current.activate("aurora-grid");
    });
    expect(result.current.operation).toBe("activate");

    await act(async () => {
      await vi.advanceTimersByTimeAsync(120_001);
    });

    expect(result.current.operation).toBeNull();
    expect(result.current.error).toMatch(/超时|重新尝试/);
    unmount();
  });

  it("restores the official appearance through the narrow restore operation", async () => {
    vi.mocked(themeApi.getSnapshot).mockResolvedValue(validSnapshot);
    vi.mocked(themeApi.restore).mockResolvedValue(receipt);
    const { result } = renderHook(() => useTheme());
    await waitFor(() => expect(result.current.snapshot).toEqual(validSnapshot));

    await act(() => result.current.restore());

    expect(themeApi.restore).toHaveBeenCalledOnce();
    expect(result.current.receipt).toEqual(receipt);
  });

  it("keeps a failed activation visible after accepting the refreshed snapshot", async () => {
    vi.mocked(themeApi.getSnapshot).mockResolvedValue(validSnapshot);
    vi.mocked(themeApi.activate).mockResolvedValue({
      operation_id: "failed-theme",
      status: "failed",
      reason_codes: ["partial-apply-failed"],
      restart_required: false,
    });
    const { result } = renderHook(() => useTheme());
    await waitFor(() => expect(result.current.snapshot).toEqual(validSnapshot));

    await act(() => result.current.activate("aurora-grid"));

    expect(result.current.error).toMatch(/未标记为已应用/);
  });

  it("imports one local image and immediately applies the generated safe theme", async () => {
    vi.mocked(themeApi.getSnapshot).mockResolvedValue(validSnapshot);
    vi.mocked(themeApi.importLocalImage).mockResolvedValue({
      theme_id: "local-0123456789abcdef",
    });
    vi.mocked(themeApi.activate).mockResolvedValue(receipt);
    const { result } = renderHook(() => useTheme());
    await waitFor(() => expect(result.current.snapshot).toEqual(validSnapshot));

    await act(() =>
      result.current.importLocalImage("My Garden", "data:image/webp;base64,UklGRgAAAABXRUJQ"),
    );

    expect(themeApi.importLocalImage).toHaveBeenCalledWith(
      "My Garden",
      "data:image/webp;base64,UklGRgAAAABXRUJQ",
    );
    expect(themeApi.activate).toHaveBeenCalledWith("local-0123456789abcdef");
    expect(result.current.receipt).toEqual(receipt);
  });
});
