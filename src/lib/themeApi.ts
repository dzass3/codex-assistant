import type {
  ForceRestartImpact,
  ThemeAsset,
  ThemeAdaptation,
  ThemeBackdrop,
  ThemeCategory,
  ThemeEffects,
  ThemeEnvironmentCheck,
  ThemeEnvironmentCheckCode,
  ThemeEnvironmentCheckState,
  ThemeEnvironmentReport,
  ThemeEnvironmentStatus,
  ThemeImportReceipt,
  ThemeMarketplaceMetadata,
  ThemeGenre,
  ThemeEditorialBadge,
  ThemeOperationReceipt,
  ThemePack,
  ThemePalette,
  ThemeReasonCode,
  ThemeRestartIntent,
  ThemeRestartMode,
  ThemeRights,
  ThemeSessionStatus,
  ThemeNextAction,
  ThemeUiSnapshot,
} from "../../shared/theme-types";
import { invoke } from "./invoke";

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
const FORBIDDEN_FIELD = /prompt|response|reasoning|command|patch|token|cookie|secret/i;
const SAFE_SLUG = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
const BUNDLED_PREVIEW = /^\/themes\/[a-z0-9][a-z0-9./-]{0,150}$/;
const LOCAL_PREVIEW = /^local-theme:[a-z0-9]+(?:-[a-z0-9]+)*$/;
const IMAGE_DATA_URL = /^data:image\/(?:jpeg|png|webp);base64,[A-Za-z0-9+/]+={0,2}$/;
const HEX = /^#[0-9a-fA-F]{6}$/;
const SHA256 = /^[0-9a-f]{64}$/;
const DATE = /^\d{4}-\d{2}-\d{2}$/;
const SESSION_STATUSES = new Set<ThemeSessionStatus>(["inactive", "paused", "ready", "degraded"]);
const OPERATION_STATUSES = new Set(["applied", "noop", "blocked", "failed"] as const);
const RESTART_INTENTS = new Set<ThemeRestartIntent>(["theme-session", "activate-theme"]);
const REASON_CODES = new Set<ThemeReasonCode>([
  "active-work",
  "monitor-uncertain",
  "unsupported-host",
  "cdp-unavailable",
  "theme-state-unavailable",
  "confirmation-required",
  "confirmation-expired",
  "impact-changed",
  "operation-conflict",
  "identity-changed",
  "termination-failed",
  "old-tree-still-running",
  "cdp-verification-failed",
  "dom-incompatible",
  "multiple-windows",
  "partial-apply-failed",
  "terminal-partial-failure",
]);
const CATEGORIES = new Set<ThemeCategory>([
  "abstract",
  "original-character",
  "project-showcase",
  "local-import",
]);
const MARKETPLACE_GENRES = new Set<ThemeGenre>([
  "anime",
  "fantasy",
  "nature",
  "cyber",
  "minimal",
  "dark",
  "space",
]);
const MARKETPLACE_BADGES = new Set<ThemeEditorialBadge>(["popular", "featured", "new"]);
const ENVIRONMENT_STATUSES = new Set<ThemeEnvironmentStatus>([
  "ready",
  "codex-not-running",
  "restart-required",
  "unsupported",
]);
const ENVIRONMENT_CHECK_CODES = new Set<ThemeEnvironmentCheckCode>([
  "supported-windows",
  "supported-architecture",
  "official-store-codex",
  "compatible-adapter",
  "single-codex-window",
  "verified-theme-session",
  "saved-theme",
]);
const ENVIRONMENT_CHECK_STATES = new Set<ThemeEnvironmentCheckState>(["pass", "action", "fail"]);
const THEME_NEXT_ACTIONS = new Set<ThemeNextAction>([
  "apply-now",
  "launch-codex-for-theme",
  "confirm-restart",
  "update-assistant",
  "use-supported-windows",
  "install-codex",
  "close-extra-windows",
  "none",
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

function uuid(value: unknown): string | null {
  return typeof value === "string" && UUID.test(value) ? value : null;
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
      ? {
          kind: "image",
          asset_id: assetId,
          overlay,
          focal_x: focalX,
          focal_y: focalY,
        }
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

function marketplace(value: unknown): ThemeMarketplaceMetadata | null {
  const raw = record(value);
  if (
    raw === null ||
    !exactKeys(raw, ["genres", "badges", "published_at", "sort_order"]) ||
    !Array.isArray(raw.genres) ||
    raw.genres.length === 0 ||
    !Array.isArray(raw.badges)
  ) {
    return null;
  }
  const genres = raw.genres.filter(
    (entry): entry is ThemeGenre =>
      typeof entry === "string" && MARKETPLACE_GENRES.has(entry as ThemeGenre),
  );
  const badges = raw.badges.filter(
    (entry): entry is ThemeEditorialBadge =>
      typeof entry === "string" && MARKETPLACE_BADGES.has(entry as ThemeEditorialBadge),
  );
  const sortOrder = integer(raw.sort_order, 1, 10_000);
  if (
    genres.length !== raw.genres.length ||
    badges.length !== raw.badges.length ||
    new Set(genres).size !== genres.length ||
    new Set(badges).size !== badges.length ||
    typeof raw.published_at !== "string" ||
    !DATE.test(raw.published_at) ||
    sortOrder === null
  ) {
    return null;
  }
  return {
    genres,
    badges,
    published_at: raw.published_at,
    sort_order: sortOrder,
  };
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

function adaptation(value: unknown): ThemeAdaptation | null {
  const raw = record(value);
  if (raw === null || !exactKeys(raw, ["luminance", "complexity", "saturation"])) return null;
  const luminance = integer(raw.luminance, 0, 100);
  const complexity = integer(raw.complexity, 0, 100);
  const saturation = integer(raw.saturation, 0, 100);
  return luminance !== null && complexity !== null && saturation !== null
    ? { luminance, complexity, saturation }
    : null;
}

function rights(value: unknown, category: ThemeCategory): ThemeRights | null {
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
  const verifiedBundled =
    category !== "local-import" &&
    raw.commercial_redistribution === true &&
    raw.status === "verified";
  const localOnly =
    category === "local-import" &&
    raw.commercial_redistribution === false &&
    raw.status === "local-only";
  return source !== null &&
    rightsholder !== null &&
    license !== null &&
    attribution !== null &&
    typeof raw.reviewed_at === "string" &&
    DATE.test(raw.reviewed_at) &&
    raw.manual_signoff === true &&
    (verifiedBundled || localOnly)
    ? {
        source,
        rightsholder,
        license,
        commercial_redistribution: raw.commercial_redistribution as boolean,
        attribution,
        reviewed_at: raw.reviewed_at,
        manual_signoff: true,
        status: raw.status as ThemeRights["status"],
      }
    : null;
}

function pack(value: unknown): ThemePack | null {
  const raw = record(value);
  const packKeys = [
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
  ] as const;
  if (raw === null) return null;
  const expectedKeys = [
    ...packKeys,
    ...(raw.marketplace === undefined ? [] : ["marketplace"]),
    ...(raw.adaptation === undefined ? [] : ["adaptation"]),
  ];
  if (!exactKeys(raw, expectedKeys)) return null;
  const id = slug(raw.id);
  const name = boundedText(raw.name, 80);
  const description = boundedText(raw.description, 240);
  const category =
    typeof raw.category === "string" && CATEGORIES.has(raw.category as ThemeCategory)
      ? (raw.category as ThemeCategory)
      : null;
  const previewPath =
    typeof raw.preview_path === "string" && category !== null
      ? category === "local-import"
        ? LOCAL_PREVIEW.test(raw.preview_path) && raw.preview_path === `local-theme:${id}`
          ? raw.preview_path
          : null
        : BUNDLED_PREVIEW.test(raw.preview_path) && !raw.preview_path.includes("..")
          ? raw.preview_path
          : null
      : null;
  const parsedBackdrop = backdrop(raw.backdrop);
  const parsedPalette = palette(raw.palette);
  const parsedEffects = effects(raw.effects);
  const parsedAdaptation = raw.adaptation === undefined ? null : adaptation(raw.adaptation);
  const parsedMarketplace = raw.marketplace === undefined ? null : marketplace(raw.marketplace);
  const assets = Array.isArray(raw.assets) ? raw.assets.map(asset) : null;
  const parsedRights = category === null ? null : rights(raw.rights, category);
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
    (raw.adaptation !== undefined && parsedAdaptation === null) ||
    (raw.marketplace !== undefined && parsedMarketplace === null) ||
    (category === "local-import" && parsedMarketplace !== null) ||
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
    ...(parsedAdaptation === null ? {} : { adaptation: parsedAdaptation }),
    assets: parsedAssets,
    rights: parsedRights,
    ...(parsedMarketplace === null ? {} : { marketplace: parsedMarketplace }),
  };
}

export function toThemeUiSnapshot(value: unknown): ThemeUiSnapshot | null {
  const raw = record(value);
  if (
    raw === null ||
    !(
      exactKeys(raw, [
        "contract_version",
        "session_status",
        "selected_theme_id",
        "applied_theme_id",
        "packs",
      ]) ||
      exactKeys(raw, [
        "contract_version",
        "session_status",
        "selected_theme_id",
        "applied_theme_id",
        "catalog_notice",
        "packs",
      ])
    )
  )
    return null;
  const sessionStatus =
    typeof raw.session_status === "string" &&
    SESSION_STATUSES.has(raw.session_status as ThemeSessionStatus)
      ? (raw.session_status as ThemeSessionStatus)
      : null;
  const selectedThemeId = raw.selected_theme_id === null ? null : slug(raw.selected_theme_id);
  const appliedThemeId = raw.applied_theme_id === null ? null : slug(raw.applied_theme_id);
  const catalogNotice =
    raw.catalog_notice === undefined || raw.catalog_notice === null
      ? null
      : boundedText(raw.catalog_notice, 80);
  const packs = Array.isArray(raw.packs) ? raw.packs.map(pack) : null;
  if (
    raw.contract_version !== 2 ||
    sessionStatus === null ||
    (selectedThemeId === null && raw.selected_theme_id !== null) ||
    (appliedThemeId === null && raw.applied_theme_id !== null) ||
    (catalogNotice === null && raw.catalog_notice !== undefined && raw.catalog_notice !== null) ||
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
    catalog_notice: catalogNotice,
    packs: parsedPacks,
  };
}

export function toThemeEnvironmentReport(value: unknown): ThemeEnvironmentReport | null {
  const raw = record(value);
  if (
    raw === null ||
    !exactKeys(raw, [
      "contract_version",
      "status",
      "checks",
      "os_build",
      "architecture",
      "codex_version",
      "verified_process_count",
      "session_reachable",
      "selected_theme_id",
      "next_action",
      "can_apply_now",
    ])
  ) {
    return null;
  }
  const status =
    typeof raw.status === "string" && ENVIRONMENT_STATUSES.has(raw.status as ThemeEnvironmentStatus)
      ? (raw.status as ThemeEnvironmentStatus)
      : null;
  const codexVersion = raw.codex_version === null ? null : boundedVersion(raw.codex_version);
  const osBuild = raw.os_build === null ? null : integer(raw.os_build, 1, 999_999);
  const architecture = ["x64", "arm64", "unsupported"].includes(String(raw.architecture))
    ? (raw.architecture as ThemeEnvironmentReport["architecture"])
    : null;
  const processCount = integer(raw.verified_process_count, 0, 16);
  const selectedThemeId = raw.selected_theme_id === null ? null : slug(raw.selected_theme_id);
  const nextAction =
    typeof raw.next_action === "string" &&
    THEME_NEXT_ACTIONS.has(raw.next_action as ThemeNextAction)
      ? (raw.next_action as ThemeNextAction)
      : null;
  const checks = Array.isArray(raw.checks) ? raw.checks.map(environmentCheck) : null;
  if (
    raw.contract_version !== 2 ||
    status === null ||
    (codexVersion === null && raw.codex_version !== null) ||
    (osBuild === null && raw.os_build !== null) ||
    architecture === null ||
    processCount === null ||
    typeof raw.session_reachable !== "boolean" ||
    (selectedThemeId === null && raw.selected_theme_id !== null) ||
    nextAction === null ||
    typeof raw.can_apply_now !== "boolean" ||
    checks === null ||
    checks.some((check) => check === null) ||
    checks.length !== ENVIRONMENT_CHECK_CODES.size ||
    new Set(checks.map((check) => check?.code)).size !== checks.length
  ) {
    return null;
  }
  return {
    contract_version: 2,
    status,
    checks: checks as ThemeEnvironmentCheck[],
    os_build: osBuild,
    architecture,
    codex_version: codexVersion,
    verified_process_count: processCount,
    session_reachable: raw.session_reachable,
    selected_theme_id: selectedThemeId,
    next_action: nextAction,
    can_apply_now: raw.can_apply_now,
  };
}

function boundedVersion(value: unknown): string | null {
  return typeof value === "string" && /^\d{1,6}(?:\.\d{1,6}){3}$/.test(value) ? value : null;
}

function environmentCheck(value: unknown): ThemeEnvironmentCheck | null {
  const raw = record(value);
  if (raw === null || !exactKeys(raw, ["code", "state"])) return null;
  const code =
    typeof raw.code === "string" &&
    ENVIRONMENT_CHECK_CODES.has(raw.code as ThemeEnvironmentCheckCode)
      ? (raw.code as ThemeEnvironmentCheckCode)
      : null;
  const state =
    typeof raw.state === "string" &&
    ENVIRONMENT_CHECK_STATES.has(raw.state as ThemeEnvironmentCheckState)
      ? (raw.state as ThemeEnvironmentCheckState)
      : null;
  return code !== null && state !== null ? { code, state } : null;
}

async function mutation(result: Promise<unknown>): Promise<ThemeOperationReceipt> {
  const receipt = toThemeOperationReceipt(await result);
  if (receipt === null) throw new Error("Theme engine returned a malformed operation receipt");
  return receipt;
}

export function toThemeOperationReceipt(value: unknown): ThemeOperationReceipt | null {
  const raw = record(value);
  if (
    raw === null ||
    !exactKeys(raw, ["operation_id", "status", "reason_codes", "restart_required"])
  ) {
    return null;
  }
  const operationId = uuid(raw.operation_id);
  const status = typeof raw.status === "string" ? raw.status : null;
  const reasonCodes = Array.isArray(raw.reason_codes) ? raw.reason_codes : null;
  if (
    operationId === null ||
    status === null ||
    !OPERATION_STATUSES.has(status as "applied" | "noop" | "blocked" | "failed") ||
    reasonCodes === null ||
    reasonCodes.some(
      (reason) => typeof reason !== "string" || !REASON_CODES.has(reason as ThemeReasonCode),
    ) ||
    new Set(reasonCodes).size !== reasonCodes.length ||
    typeof raw.restart_required !== "boolean"
  ) {
    return null;
  }
  return {
    operation_id: operationId,
    status: status as ThemeOperationReceipt["status"],
    reason_codes: reasonCodes as ThemeReasonCode[],
    restart_required: raw.restart_required,
  };
}

export function toForceRestartImpact(value: unknown): ForceRestartImpact | null {
  const raw = record(value);
  if (
    raw === null ||
    !exactKeys(raw, [
      "confirmation_ticket",
      "intent",
      "active_work_count",
      "monitor_confident",
      "grace_period_ms",
      "expires_at_ms",
    ])
  ) {
    return null;
  }
  const confirmationTicket = uuid(raw.confirmation_ticket);
  const intent = typeof raw.intent === "string" ? raw.intent : null;
  const activeWorkCount = integer(raw.active_work_count, 0, 10_000);
  const expiresAtMs = integer(raw.expires_at_ms, 0, Number.MAX_SAFE_INTEGER);
  if (
    confirmationTicket === null ||
    intent === null ||
    !RESTART_INTENTS.has(intent as ThemeRestartIntent) ||
    activeWorkCount === null ||
    typeof raw.monitor_confident !== "boolean" ||
    (activeWorkCount === 0 && raw.monitor_confident !== false) ||
    raw.grace_period_ms !== 5_000 ||
    expiresAtMs === null
  ) {
    return null;
  }
  return {
    confirmation_ticket: confirmationTicket,
    intent: intent as ThemeRestartIntent,
    active_work_count: activeWorkCount,
    monitor_confident: raw.monitor_confident,
    grace_period_ms: 5_000,
    expires_at_ms: expiresAtMs,
  };
}

export const themeApi = {
  async getEnvironment(): Promise<ThemeEnvironmentReport | null> {
    return toThemeEnvironmentReport(await invoke("get_theme_environment"));
  },
  async getSnapshot(): Promise<ThemeUiSnapshot | null> {
    return toThemeUiSnapshot(await invoke("get_theme_snapshot"));
  },
  async getPreviewDataUrl(themeId: string): Promise<string | null> {
    if (slug(themeId) === null) throw new Error("Invalid theme identifier");
    const value = await invoke("get_theme_preview_data_url", { themeId });
    return typeof value === "string" && value.length <= 2_800_000 && IMAGE_DATA_URL.test(value)
      ? value
      : null;
  },
  async importLocalImage(name: string, imageDataUrl: string): Promise<ThemeImportReceipt> {
    const safeName = boundedText(name, 80);
    if (
      safeName === null ||
      imageDataUrl.length > 2_100_000 ||
      !IMAGE_DATA_URL.test(imageDataUrl)
    ) {
      throw new Error("Invalid local theme image");
    }
    const value = record(await invoke("import_local_theme", { name: safeName, imageDataUrl }));
    const themeId = value === null || !exactKeys(value, ["theme_id"]) ? null : slug(value.theme_id);
    if (themeId === null || !themeId.startsWith("local-")) {
      throw new Error("Malformed local theme import receipt");
    }
    return { theme_id: themeId };
  },
  async prepareForceRestart(
    intent: ThemeRestartIntent,
    themeId?: string,
  ): Promise<ForceRestartImpact> {
    const impact = toForceRestartImpact(
      await invoke("prepare_force_restart", {
        intent,
        subject: themeId ?? null,
      }),
    );
    if (impact === null) throw new Error("Malformed force restart impact");
    return impact;
  },
  cancelForceRestart: (confirmationTicket: string) =>
    invoke("cancel_force_restart", { confirmationTicket }).then((value) => value === true),
  startSession: (restartMode: ThemeRestartMode = "safe", confirmationTicket?: string) =>
    mutation(
      invoke("start_theme_session", {
        restartMode,
        confirmationTicket: confirmationTicket ?? null,
      }),
    ),
  activate(themeId: string, restartMode: ThemeRestartMode = "safe", confirmationTicket?: string) {
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
