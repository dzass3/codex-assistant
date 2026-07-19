import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "./invoke";
import { themeApi, toThemeUiSnapshot } from "./themeApi";

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

  it.each([
    { ...snapshot, prompt: "CANARY_PRIVATE_PROMPT" },
    { ...snapshot, session_status: "unknown" },
    { ...snapshot, applied_theme_id: "missing-pack" },
    { ...snapshot, packs: [{ ...snapshot.packs[0], category: "celebrity" }] },
    { ...snapshot, packs: [{ ...snapshot.packs[0], preview_path: "https://remote/theme.jpg" }] },
    {
      ...snapshot,
      packs: [{ ...snapshot.packs[0], rights: { ...verifiedRights, status: "local-only" } }],
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

  it("uses exactly four narrow commands and one-click activation with a bounded identifier", async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce(snapshot)
      .mockResolvedValueOnce(receipt)
      .mockResolvedValueOnce({
        confirmation_ticket: "d2719d93-b823-4a7f-934f-23cbe01c8ab1",
        intent: "activate-theme",
        active_native_children: 2,
        grace_period_ms: 5000,
        expires_at_ms: 100000,
      })
      .mockResolvedValueOnce(receipt)
      .mockResolvedValueOnce(receipt);

    await expect(themeApi.getSnapshot()).resolves.toEqual(snapshot);
    await expect(themeApi.startSession()).resolves.toEqual(receipt);
    await expect(
      themeApi.prepareForceRestart("activate-theme", "observatory-muse"),
    ).resolves.toMatchObject({ active_native_children: 2 });
    await expect(themeApi.activate("observatory-muse")).resolves.toEqual(receipt);
    await expect(themeApi.restore()).resolves.toEqual(receipt);

    expect(vi.mocked(invoke).mock.calls).toEqual([
      ["get_theme_snapshot"],
      ["start_theme_session", { restartMode: "safe", confirmationTicket: null }],
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
  });
});
