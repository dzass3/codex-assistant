import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "./invoke";
import { themeApi, toThemeEnvironmentReport, toThemeUiSnapshot } from "./themeApi";

vi.mock("./invoke", () => ({ invoke: vi.fn() }));

const verifiedRights = {
  source: "Original abstract theme authored for Codex Assistant",
  rightsholder: "Codex Assistant project",
  license: "Project-owned distribution asset",
  commercial_redistribution: true,
  attribution: "Original artwork created for Codex Assistant",
  reviewed_at: "2026-07-18",
  manual_signoff: true,
  status: "verified",
};

const snapshot = {
  contract_version: 2,
  session_status: "ready",
  selected_theme_id: "aurora-grid",
  applied_theme_id: "aurora-grid",
  catalog_notice: null,
  packs: [
    {
      schema_version: 1,
      minimum_engine_version: 1,
      id: "aurora-grid",
      name: "Aurora Grid",
      description: "Project-owned abstract aurora with restrained glass surfaces.",
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
      rights: verifiedRights,
    },
    {
      schema_version: 1,
      minimum_engine_version: 1,
      id: "observatory-muse",
      name: "Observatory Muse",
      description: "Original fictional technologist in a quiet violet observatory.",
      category: "original-character",
      preview_path: "/themes/original-observatory-muse.jpg",
      backdrop: {
        kind: "image",
        asset_id: "original-observatory-muse",
        overlay: "#071326",
        focal_x: 50,
        focal_y: 50,
      },
      palette: {
        surface: "#0c1730",
        surface_strong: "#101a35",
        text: "#f4f2ff",
        accent: "#a990ff",
        border: "#7f8cff",
      },
      effects: {
        surface_opacity: 76,
        blur_px: 24,
        contrast_percent: 105,
        motion: false,
      },
      assets: [
        {
          id: "original-observatory-muse",
          mime_type: "image/jpeg",
          sha256: "a486add3d4c8efb5c8f9b28c453291c0ed2192d415565f0bec1dc6f9bdc78c3a",
        },
      ],
      rights: verifiedRights,
    },
  ],
} as const;

const receipt = {
  operation_id: "d2719d93-b823-4a7f-934f-23cbe01c8ab0",
  status: "applied",
  reason_codes: [],
  restart_required: false,
};

describe("theme snapshot boundary", () => {
  it("accepts only the exact rights-audited declarative contract", () => {
    expect(toThemeUiSnapshot(snapshot)).toEqual(snapshot);
  });

  it("accepts local-only rights only for a local-import pack", () => {
    const local = {
      ...snapshot.packs[1],
      id: "arina-pink",
      name: "Arina Pink",
      category: "local-import",
      preview_path: "local-theme:arina-pink",
      backdrop: { ...snapshot.packs[1].backdrop, asset_id: "arina-pink" },
      assets: [{ ...snapshot.packs[1].assets[0], id: "arina-pink" }],
      rights: {
        ...verifiedRights,
        commercial_redistribution: false,
        status: "local-only",
      },
    } as const;

    expect(
      toThemeUiSnapshot({
        ...snapshot,
        selected_theme_id: null,
        applied_theme_id: null,
        packs: [local],
      })?.packs[0],
    ).toEqual(local);
    expect(
      toThemeUiSnapshot({
        ...snapshot,
        selected_theme_id: null,
        applied_theme_id: null,
        packs: [{ ...local, category: "abstract" }],
      }),
    ).toBeNull();
  });

  it.each([
    { ...snapshot, prompt: "CANARY_PRIVATE_PROMPT" },
    { ...snapshot, session_status: "unknown" },
    { ...snapshot, applied_theme_id: "missing-pack" },
    { ...snapshot, packs: [{ ...snapshot.packs[0], category: "celebrity" }] },
    {
      ...snapshot,
      packs: [{ ...snapshot.packs[0], preview_path: "https://remote/theme.jpg" }],
    },
    {
      ...snapshot,
      packs: [
        {
          ...snapshot.packs[0],
          rights: { ...verifiedRights, status: "local-only" },
        },
      ],
    },
    {
      ...snapshot,
      packs: [
        {
          ...snapshot.packs[1],
          assets: [{ ...snapshot.packs[1].assets[0], sha256: "not-a-hash" }],
        },
      ],
    },
    { ...snapshot, packs: [{ ...snapshot.packs[0], unknown_field: true }] },
  ])("fails closed for malformed, remote, or unverified bundled data", (value) => {
    expect(toThemeUiSnapshot(value)).toBeNull();
  });
});

describe("theme command surface", () => {
  beforeEach(() => vi.mocked(invoke).mockReset());

  it("uses narrow commands and one-click activation with bounded identifiers", async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce(snapshot)
      .mockResolvedValueOnce("data:image/jpeg;base64,YQ==")
      .mockResolvedValueOnce(receipt)
      .mockResolvedValueOnce({ theme_id: "local-0123456789abcdef" })
      .mockResolvedValueOnce({
        confirmation_ticket: "d2719d93-b823-4a7f-934f-23cbe01c8ab1",
        intent: "activate-theme",
        active_work_count: 2,
        monitor_confident: true,
        grace_period_ms: 5000,
        expires_at_ms: 100000,
      })
      .mockResolvedValueOnce(receipt)
      .mockResolvedValueOnce(receipt);

    await expect(themeApi.getSnapshot()).resolves.toEqual(snapshot);
    await expect(themeApi.getPreviewDataUrl("arina-pink")).resolves.toBe(
      "data:image/jpeg;base64,YQ==",
    );
    await expect(themeApi.startSession()).resolves.toEqual(receipt);
    await expect(
      themeApi.importLocalImage("My Garden", "data:image/webp;base64,UklGRgAAAABXRUJQ"),
    ).resolves.toEqual({ theme_id: "local-0123456789abcdef" });
    await expect(
      themeApi.prepareForceRestart("activate-theme", "observatory-muse"),
    ).resolves.toMatchObject({ active_work_count: 2, monitor_confident: true });
    await expect(themeApi.activate("observatory-muse")).resolves.toEqual(receipt);
    await expect(themeApi.restore()).resolves.toEqual(receipt);

    expect(vi.mocked(invoke).mock.calls).toEqual([
      ["get_theme_snapshot"],
      ["get_theme_preview_data_url", { themeId: "arina-pink" }],
      ["start_theme_session", { restartMode: "safe", confirmationTicket: null }],
      [
        "import_local_theme",
        {
          name: "My Garden",
          imageDataUrl: "data:image/webp;base64,UklGRgAAAABXRUJQ",
        },
      ],
      ["prepare_force_restart", { intent: "activate-theme", subject: "observatory-muse" }],
      [
        "activate_theme",
        {
          themeId: "observatory-muse",
          restartMode: "safe",
          confirmationTicket: null,
        },
      ],
      ["restore_theme"],
    ]);
    await expect(themeApi.activate("../outside")).rejects.toThrow("Invalid theme identifier");
    await expect(themeApi.getPreviewDataUrl("../outside")).rejects.toThrow(
      "Invalid theme identifier",
    );
    await expect(themeApi.importLocalImage("Remote", "https://example.com/a.webp")).rejects.toThrow(
      "Invalid local theme image",
    );
  });
});

describe("theme environment boundary", () => {
  const environment = {
    contract_version: 2,
    status: "restart-required",
    checks: [
      { code: "supported-windows", state: "pass" },
      { code: "supported-architecture", state: "pass" },
      { code: "official-store-codex", state: "pass" },
      { code: "compatible-adapter", state: "pass" },
      { code: "single-codex-window", state: "pass" },
      { code: "verified-theme-session", state: "action" },
      { code: "saved-theme", state: "pass" },
    ],
    os_build: 22621,
    architecture: "x64",
    codex_version: "26.715.8383.0",
    verified_process_count: 1,
    session_reachable: false,
    selected_theme_id: "aurora-grid",
    next_action: "confirm-restart",
    can_apply_now: false,
  } as const;

  it("accepts only the on-demand environment contract", async () => {
    expect(toThemeEnvironmentReport(environment)).toEqual(environment);
    expect(toThemeEnvironmentReport({ ...environment, status: "maybe" })).toBeNull();
    expect(toThemeEnvironmentReport({ ...environment, leaked_path: "C:\\private" })).toBeNull();
    expect(
      toThemeEnvironmentReport({ ...environment, checks: environment.checks.slice(0, 4) }),
    ).toBeNull();

    vi.mocked(invoke).mockResolvedValueOnce(environment);
    await expect(themeApi.getEnvironment()).resolves.toEqual(environment);
    expect(invoke).toHaveBeenCalledWith("get_theme_environment");

    vi.mocked(invoke).mockResolvedValueOnce({
      ...environment,
      contract_version: 1,
      launcher_installed: true,
    });
    await expect(themeApi.getEnvironment()).resolves.toBeNull();
  });
});
