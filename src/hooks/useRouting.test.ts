import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { RoutingOperationReceipt, RoutingUiSnapshot } from "../../shared/routing-types";
import { routingApi } from "../lib/routingApi";
import { useRouting } from "./useRouting";

vi.mock("../lib/routingApi", () => ({
  routingApi: {
    getSnapshot: vi.fn(),
    subscribe: vi.fn(),
    install: vi.fn(),
    restore: vi.fn(),
    requestCodexRestart: vi.fn(),
    beginPreflight: vi.fn(),
    setRootEnabled: vi.fn(),
  },
}));

const validSnapshot: RoutingUiSnapshot = {
  contract_version: 1,
  setup: {
    installation_status: "installed",
    restart_status: "not-required",
    preflight_status: "complete",
    cdp_status: "ready",
    backup_label: "routing-backup-20260718",
    config_changes: ["agents.max_depth"],
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
};

describe("useRouting", () => {
  beforeEach(() => {
    vi.mocked(routingApi.getSnapshot).mockReset();
    vi.mocked(routingApi.subscribe).mockReset();
    vi.mocked(routingApi.install).mockReset();
    vi.mocked(routingApi.restore).mockReset();
    vi.mocked(routingApi.requestCodexRestart).mockReset();
    vi.mocked(routingApi.beginPreflight).mockReset();
    vi.mocked(routingApi.setRootEnabled).mockReset();
    vi.mocked(routingApi.subscribe).mockResolvedValue(() => undefined);
  });

  it("retains the last verified snapshot when a refresh is malformed", async () => {
    vi.mocked(routingApi.getSnapshot)
      .mockResolvedValueOnce(validSnapshot)
      .mockResolvedValueOnce(null);
    const { result } = renderHook(() => useRouting());
    await waitFor(() => expect(result.current.snapshot).toEqual(validSnapshot));

    await act(() => result.current.refresh());

    expect(result.current.snapshot).toEqual(validSnapshot);
    expect(result.current.degraded).toBe(true);
    expect(result.current.error).toMatch(/上一次/);
  });

  it("exposes one install operation and refreshes after its receipt", async () => {
    let finishInstall: ((value: RoutingOperationReceipt) => void) | undefined;
    vi.mocked(routingApi.getSnapshot).mockResolvedValue(validSnapshot);
    vi.mocked(routingApi.install).mockReturnValue(
      new Promise<RoutingOperationReceipt>((resolve) => {
        finishInstall = resolve;
      }),
    );
    const { result } = renderHook(() => useRouting());
    await waitFor(() => expect(result.current.snapshot).toEqual(validSnapshot));

    let installing: Promise<unknown> | undefined;
    act(() => {
      installing = result.current.install();
    });
    expect(result.current.operation).toBe("install");
    await act(async () => {
      finishInstall?.({
        operation_id: "d2719d93-b823-4a7f-934f-23cbe01c8ab0",
        status: "applied",
        reason_codes: [],
        restart_required: true,
      });
      await installing;
    });

    expect(result.current.operation).toBeNull();
    expect(routingApi.getSnapshot).toHaveBeenCalledTimes(2);
  });

  it("refreshes the verified state after restoring owned configuration", async () => {
    vi.mocked(routingApi.getSnapshot).mockResolvedValue(validSnapshot);
    vi.mocked(routingApi.restore).mockResolvedValue({
      operation_id: "d2719d93-b823-4a7f-934f-23cbe01c8ab1",
      status: "applied",
      reason_codes: [],
      restart_required: true,
    });
    const { result } = renderHook(() => useRouting());
    await waitFor(() => expect(result.current.snapshot).toEqual(validSnapshot));

    await act(() => result.current.restore());

    expect(routingApi.restore).toHaveBeenCalledOnce();
    expect(routingApi.getSnapshot).toHaveBeenCalledTimes(2);
  });

  it("refreshes the verified state after requesting the one-time restart", async () => {
    vi.mocked(routingApi.getSnapshot).mockResolvedValue(validSnapshot);
    vi.mocked(routingApi.requestCodexRestart).mockResolvedValue({
      operation_id: "d2719d93-b823-4a7f-934f-23cbe01c8ab2",
      status: "blocked",
      reason_codes: ["active-child"],
      restart_required: true,
    });
    const { result } = renderHook(() => useRouting());
    await waitFor(() => expect(result.current.snapshot).toEqual(validSnapshot));

    await act(() => result.current.requestRestart());

    expect(routingApi.requestCodexRestart).toHaveBeenCalledOnce();
    expect(result.current.receipt?.reason_codes).toEqual(["active-child"]);
  });

  it("begins native preflight for the selected visible root", async () => {
    vi.mocked(routingApi.getSnapshot).mockResolvedValue(validSnapshot);
    vi.mocked(routingApi.beginPreflight).mockResolvedValue({
      operation_id: "d2719d93-b823-4a7f-934f-23cbe01c8ab3",
      status: "applied",
      reason_codes: [],
      restart_required: false,
    });
    const { result } = renderHook(() => useRouting());
    await waitFor(() => expect(result.current.snapshot).toEqual(validSnapshot));

    await act(() => result.current.beginPreflight("d2719d93-b823-4a7f-934f-23cbe01c8ab0"));

    expect(routingApi.beginPreflight).toHaveBeenCalledWith("d2719d93-b823-4a7f-934f-23cbe01c8ab0");
  });

  it("changes routing only for the selected root task", async () => {
    vi.mocked(routingApi.getSnapshot).mockResolvedValue(validSnapshot);
    vi.mocked(routingApi.setRootEnabled).mockResolvedValue({
      operation_id: "d2719d93-b823-4a7f-934f-23cbe01c8ab4",
      status: "applied",
      reason_codes: [],
      restart_required: false,
    });
    const { result } = renderHook(() => useRouting());
    await waitFor(() => expect(result.current.snapshot).toEqual(validSnapshot));

    await act(() => result.current.setRootEnabled("d2719d93-b823-4a7f-934f-23cbe01c8ab0", true));

    expect(routingApi.setRootEnabled).toHaveBeenCalledWith(
      "d2719d93-b823-4a7f-934f-23cbe01c8ab0",
      true,
    );
  });
});
