import type {
  ThemeAsset,
  ThemeBackdrop,
  ThemeCategory,
  ThemeEffects,
  ThemeOperationReceipt,
  ThemePack,
  ThemePalette,
  ThemeRights,
  ThemeSessionStatus,
  ThemeUiSnapshot,
} from "../../shared/theme-types";
import type { ForceRestartImpact, RestartIntent, RestartMode } from "../../shared/routing-types";
import { toForceRestartImpact } from "./routingApi";
import { invoke } from "./invoke";
import { toRoutingOperationReceipt } from "./routingApi";

const FORBIDDEN_FIELD = /prompt|response|reasoning|command|patch|token|cookie|secret/i;
const SAFE_SLUG = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
const LOCAL_PREVIEW = /^\/themes\/[a-z0-9][a-z0-9./-]{0,150}$/;
const HEX = /^#[0-9a-fA-F]{6}$/;
const SHA256 = /^[0-9a-f]{64}$/;
const DATE = /^\d{4}-\d{2}-\d{2}$/;
const SESSION_STATUSES = new Set<ThemeSessionStatus>(["inactive", "ready", "degraded"]);
const CATEGORIES = new Set<ThemeCategory>([
  "abstract",
  "original-character",
  "project-showcase",
  "local-import",
]);

function record(value: unknown): Record<string, unknown> | null {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

function exactKeys(value: Record<string, unknown>, expected: readonly string[]): boolean {
  const keys = Object.keys(value);
  return (
    keys.length === expected.length &&
    keys.every((key) => expected.includes(key) && !FORBIDDEN_FIELD.test(key))
  );
}

function boundedText(value: unknown, maximum: number): string | null {
  if (typeof value !== "string" || value.length === 0 || value.length > maximum) return null;
  const lower = value.toLowerCase();
  const hasControlCharacter = Array.from(value).some((character) => {
    const code = character.codePointAt(0) ?? 0;
    return code < 32 || code === 127;
  });
  return hasControlCharacter ||
    lower.includes("javascript:") ||
    lower.includes("<script") ||
    /https?:\/\//i.test(value)
    ? null
    : value;
}

function slug(value: unknown): string | null {
  return typeof value === "string" && value.length <= 80 && SAFE_SLUG.test(value) ? value : null;
}

function integer(value: unknown, minimum: number, maximum: number): number | null {
  return typeof value === "number" &&
    Number.isSafeInteger(value) &&
    value >= minimum &&
    value <= maximum
    ? value
    : null;
}

function color(value: unknown): string | null {
  return typeof value === "string" && HEX.test(value) ? value : null;
}

function backdrop(value: unknown): ThemeBackdrop | null {
  const raw = record(value);
  if (raw === null || typeof raw.kind !== "string") return null;
  if (raw.kind === "gradient") {
    if (
      !exactKeys(raw, ["kind", "angle", "colors"]) ||
      !Array.isArray(raw.colors) ||
      raw.colors.length !== 3
    )
      return null;
    const angle = integer(raw.angle, 0, 360);
    const colors = raw.colors.map(color);
    return angle !== null && colors.every((entry): entry is string => entry !== null)
      ? { kind: "gradient", angle, colors: colors as [string, string, string] }
      : null;
  }
  if (raw.kind === "image") {
    if (!exactKeys(raw, ["kind", "asset_id", "overlay", "focal_x", "focal_y"])) return null;
    const assetId = slug(raw.asset_id);
    const overlay = color(raw.overlay);
    const focalX = integer(raw.focal_x, 0, 100);
    const focalY = integer(raw.focal_y, 0, 100);
    return assetId !== null && overlay !== null && focalX !== null && focalY !== null
      ? { kind: "image", asset_id: assetId, overlay, focal_x: focalX, focal_y: focalY }
      : null;
  }
  return null;
}

function palette(value: unknown): ThemePalette | null {
  const raw = record(value);
  if (raw === null || !exactKeys(raw, ["surface", "surface_strong", "text", "accent", "border"]))
    return null;
  const parsed = {
    surface: color(raw.surface),
    surface_strong: color(raw.surface_strong),
    text: color(raw.text),
    accent: color(raw.accent),
    border: color(raw.border),
  };
  return Object.values(parsed).every((entry): entry is string => entry !== null)
    ? (parsed as ThemePalette)
    : null;
}

function effects(value: unknown): ThemeEffects | null {
  const raw = record(value);
  if (raw === null || !exactKeys(raw, ["surface_opacity", "blur_px", "contrast_percent", "motion"]))
    return null;
  const surfaceOpacity = integer(raw.surface_opacity, 25, 100);
  const blurPx = integer(raw.blur_px, 0, 40);
  const contrastPercent = integer(raw.contrast_percent, 80, 140);
  return surfaceOpacity !== null &&
    blurPx !== null &&
    contrastPercent !== null &&
    typeof raw.motion === "boolean"
    ? {
        surface_opacity: surfaceOpacity,
        blur_px: blurPx,
        contrast_percent: contrastPercent,
        motion: raw.motion,
      }
    : null;
}

function asset(value: unknown): ThemeAsset | null {
  const raw = record(value);
  if (raw === null || !exactKeys(raw, ["id", "mime_type", "sha256"])) return null;
  const id = slug(raw.id);
  return id !== null &&
    (raw.mime_type === "image/jpeg" ||
      raw.mime_type === "image/png" ||
      raw.mime_type === "image/webp") &&
    typeof raw.sha256 === "string" &&
    SHA256.test(raw.sha256)
    ? { id, mime_type: raw.mime_type, sha256: raw.sha256 }
    : null;
}

function rights(value: unknown): ThemeRights | null {
  const raw = record(value);
  if (
    raw === null ||
    !exactKeys(raw, [
      "source",
      "rightsholder",
      "license",
      "commercial_redistribution",
      "attribution",
      "reviewed_at",
      "manual_signoff",
      "status",
    ])
  )
    return null;
  const source = boundedText(raw.source, 240);
  const rightsholder = boundedText(raw.rightsholder, 120);
  const license = boundedText(raw.license, 120);
  const attribution = boundedText(raw.attribution, 240);
  return source !== null &&
    rightsholder !== null &&
    license !== null &&
    attribution !== null &&
    raw.commercial_redistribution === true &&
    typeof raw.reviewed_at === "string" &&
    DATE.test(raw.reviewed_at) &&
    raw.manual_signoff === true &&
    raw.status === "verified"
    ? {
        source,
        rightsholder,
        license,
        commercial_redistribution: true,
        attribution,
        reviewed_at: raw.reviewed_at,
        manual_signoff: true,
        status: "verified",
      }
    : null;
}

function pack(value: unknown): ThemePack | null {
  const raw = record(value);
  if (
    raw === null ||
    !exactKeys(raw, [
      "schema_version",
      "minimum_engine_version",
      "id",
      "name",
      "description",
      "category",
      "preview_path",
      "backdrop",
      "palette",
      "effects",
      "assets",
      "rights",
    ])
  )
    return null;
  const id = slug(raw.id);
  const name = boundedText(raw.name, 80);
  const description = boundedText(raw.description, 240);
  const category =
    typeof raw.category === "string" && CATEGORIES.has(raw.category as ThemeCategory)
      ? (raw.category as ThemeCategory)
      : null;
  const previewPath =
    typeof raw.preview_path === "string" &&
    LOCAL_PREVIEW.test(raw.preview_path) &&
    !raw.preview_path.includes("..")
      ? raw.preview_path
      : null;
  const parsedBackdrop = backdrop(raw.backdrop);
  const parsedPalette = palette(raw.palette);
  const parsedEffects = effects(raw.effects);
  const assets = Array.isArray(raw.assets) ? raw.assets.map(asset) : null;
  const parsedRights = rights(raw.rights);
  const minimumEngineVersion = integer(raw.minimum_engine_version, 1, 1);
  if (
    raw.schema_version !== 1 ||
    minimumEngineVersion === null ||
    id === null ||
    name === null ||
    description === null ||
    category === null ||
    previewPath === null ||
    parsedBackdrop === null ||
    parsedPalette === null ||
    parsedEffects === null ||
    assets === null ||
    assets.some((entry) => entry === null) ||
    parsedRights === null
  )
    return null;
  const parsedAssets = assets as ThemeAsset[];
  if (new Set(parsedAssets.map((entry) => entry.id)).size !== parsedAssets.length) return null;
  if (
    parsedBackdrop.kind === "image" &&
    !parsedAssets.some((entry) => entry.id === parsedBackdrop.asset_id)
  )
    return null;
  return {
    schema_version: 1,
    minimum_engine_version: minimumEngineVersion,
    id,
    name,
    description,
    category,
    preview_path: previewPath,
    backdrop: parsedBackdrop,
    palette: parsedPalette,
    effects: parsedEffects,
    assets: parsedAssets,
    rights: parsedRights,
  };
}

export function toThemeUiSnapshot(value: unknown): ThemeUiSnapshot | null {
  const raw = record(value);
  if (
    raw === null ||
    !exactKeys(raw, [
      "contract_version",
      "session_status",
      "selected_theme_id",
      "applied_theme_id",
      "packs",
    ])
  )
    return null;
  const sessionStatus =
    typeof raw.session_status === "string" &&
    SESSION_STATUSES.has(raw.session_status as ThemeSessionStatus)
      ? (raw.session_status as ThemeSessionStatus)
      : null;
  const selectedThemeId = raw.selected_theme_id === null ? null : slug(raw.selected_theme_id);
  const appliedThemeId = raw.applied_theme_id === null ? null : slug(raw.applied_theme_id);
  const packs = Array.isArray(raw.packs) ? raw.packs.map(pack) : null;
  if (
    raw.contract_version !== 2 ||
    sessionStatus === null ||
    (selectedThemeId === null && raw.selected_theme_id !== null) ||
    (appliedThemeId === null && raw.applied_theme_id !== null) ||
    packs === null ||
    packs.some((entry) => entry === null)
  )
    return null;
  const parsedPacks = packs as ThemePack[];
  if (
    new Set(parsedPacks.map((entry) => entry.id)).size !== parsedPacks.length ||
    (selectedThemeId !== null && !parsedPacks.some((entry) => entry.id === selectedThemeId)) ||
    (appliedThemeId !== null && !parsedPacks.some((entry) => entry.id === appliedThemeId))
  )
    return null;
  return {
    contract_version: 2,
    session_status: sessionStatus,
    selected_theme_id: selectedThemeId,
    applied_theme_id: appliedThemeId,
    packs: parsedPacks,
  };
}

async function mutation(result: Promise<unknown>): Promise<ThemeOperationReceipt> {
  const receipt = toRoutingOperationReceipt(await result);
  if (receipt === null) throw new Error("Theme engine returned a malformed operation receipt");
  return receipt;
}

export const themeApi = {
  async getSnapshot(): Promise<ThemeUiSnapshot | null> {
    return toThemeUiSnapshot(await invoke("get_theme_snapshot"));
  },
  async prepareForceRestart(intent: RestartIntent, themeId?: string): Promise<ForceRestartImpact> {
    const impact = toForceRestartImpact(
      await invoke("prepare_force_restart", { intent, subject: themeId ?? null }),
    );
    if (impact === null) throw new Error("Malformed force restart impact");
    return impact;
  },
  cancelForceRestart: (confirmationTicket: string) =>
    invoke("cancel_force_restart", { confirmationTicket }).then((value) => value === true),
  startSession: (restartMode: RestartMode = "safe", confirmationTicket?: string) =>
    mutation(
      invoke("start_theme_session", {
        restartMode,
        confirmationTicket: confirmationTicket ?? null,
      }),
    ),
  activate(themeId: string, restartMode: RestartMode = "safe", confirmationTicket?: string) {
    if (slug(themeId) === null) return Promise.reject(new Error("Invalid theme identifier"));
    return mutation(
      invoke("activate_theme", {
        themeId,
        restartMode,
        confirmationTicket: confirmationTicket ?? null,
      }),
    );
  },
  restore: () => mutation(invoke("restore_theme")),
};
