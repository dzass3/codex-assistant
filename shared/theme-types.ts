import type { RoutingOperationReceipt } from "./routing-types";

export type ThemeSessionStatus = "inactive" | "paused" | "ready" | "degraded";
export type ThemeCategory = "abstract" | "original-character" | "project-showcase" | "local-import";
export type ThemeRightsStatus = "verified" | "local-only" | "rejected";

export interface ThemeGradientBackdrop {
  kind: "gradient";
  angle: number;
  colors: [string, string, string];
}

export interface ThemeImageBackdrop {
  kind: "image";
  asset_id: string;
  overlay: string;
  focal_x: number;
  focal_y: number;
}

export type ThemeBackdrop = ThemeGradientBackdrop | ThemeImageBackdrop;

export interface ThemePalette {
  surface: string;
  surface_strong: string;
  text: string;
  accent: string;
  border: string;
}

export interface ThemeEffects {
  surface_opacity: number;
  blur_px: number;
  contrast_percent: number;
  motion: boolean;
}

export interface ThemeAsset {
  id: string;
  mime_type: "image/jpeg" | "image/png" | "image/webp";
  sha256: string;
}

export interface ThemeRights {
  source: string;
  rightsholder: string;
  license: string;
  commercial_redistribution: boolean;
  attribution: string;
  reviewed_at: string;
  manual_signoff: boolean;
  status: ThemeRightsStatus;
}

export interface ThemePack {
  schema_version: 1;
  minimum_engine_version: number;
  id: string;
  name: string;
  description: string;
  category: ThemeCategory;
  preview_path: string;
  backdrop: ThemeBackdrop;
  palette: ThemePalette;
  effects: ThemeEffects;
  assets: ThemeAsset[];
  rights: ThemeRights;
}

export interface ThemeUiSnapshot {
  contract_version: 2;
  session_status: ThemeSessionStatus;
  selected_theme_id: string | null;
  applied_theme_id: string | null;
  packs: ThemePack[];
}

export type ThemeOperationReceipt = RoutingOperationReceipt;
