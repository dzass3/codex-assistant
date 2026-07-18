import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ThemeOperationReceipt, ThemeUiSnapshot } from "../../shared/theme-types";
import { themeApi } from "../lib/themeApi";
import { useTheme } from "./useTheme";

vi.mock("../lib/themeApi", () => ({
  themeApi: {
    getSnapshot: vi.fn(),
    startSession: vi.fn(),
    apply: vi.fn(),
    restore: vi.fn(),
  },
}));

const validSnapshot: ThemeUiSnapshot = {
  contract_version: 1,
  session_status: "ready",
  active_theme_id: null,
  packs: [],
};

const receipt: ThemeOperationReceipt = {
  operation_id: "d2719d93-b823-4a7f-934f-23cbe01c8ab0",
  status: "applied",
  reason_codes: [],
  restart_required: false,
};

describe("useTheme", () => {
  beforeEach(() => {
    vi.mocked(themeApi.getSnapshot).mockReset();
    vi.mocked(themeApi.startSession).mockReset();
    vi.mocked(themeApi.apply).mockReset();
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

  it("serializes session and apply mutations then refreshes confirmed state", async () => {
    let finishStart: ((value: ThemeOperationReceipt) => void) | undefined;
    vi.mocked(themeApi.getSnapshot).mockResolvedValue(validSnapshot);
    vi.mocked(themeApi.startSession).mockReturnValue(
      new Promise<ThemeOperationReceipt>((resolve) => {
        finishStart = resolve;
      }),
    );
    vi.mocked(themeApi.apply).mockResolvedValue(receipt);
    const { result } = renderHook(() => useTheme());
    await waitFor(() => expect(result.current.snapshot).toEqual(validSnapshot));

    let starting: Promise<unknown> | undefined;
    act(() => {
      starting = result.current.startSession();
    });
    expect(result.current.operation).toBe("start-session");
    await expect(result.current.apply("aurora-grid")).resolves.toBeNull();
    expect(themeApi.apply).not.toHaveBeenCalled();

    await act(async () => {
      finishStart?.(receipt);
      await starting;
    });
    await act(() => result.current.apply("aurora-grid"));

    expect(themeApi.startSession).toHaveBeenCalledOnce();
    expect(themeApi.apply).toHaveBeenCalledWith("aurora-grid");
    expect(themeApi.getSnapshot).toHaveBeenCalledTimes(3);
    expect(result.current.operation).toBeNull();
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
});
