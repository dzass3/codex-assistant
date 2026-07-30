import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, isAbsolute, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import { isValidThemeMarketplace } from "./theme-marketplace-validation.mjs";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const outputRoot = repositoryRoot;
const planPath = join(repositoryRoot, "assets/theme-sources/catalog-plan.json");
const builder = join(repositoryRoot, "scripts/themes/build-bundled-theme.mjs");
const plan = JSON.parse(readFileSync(planPath, "utf8"));

if (
  plan?.schema_version !== 1 ||
  !Array.isArray(plan.themes) ||
  plan.themes.length !== 12 ||
  new Set(plan.themes.map((theme) => theme.id)).size !== 12
) {
  throw new Error("approved theme catalog plan must contain exactly 12 unique themes");
}

const temporaryDirectory = mkdtempSync(join(tmpdir(), "codex-assistant-theme-catalog-"));
try {
  const emptyCatalog = join(temporaryDirectory, "catalog.json");
  writeFileSync(emptyCatalog, '{\n  "schema_version": 1,\n  "themes": []\n}\n');
  let catalogTemplate = emptyCatalog;

  for (const theme of plan.themes) {
    validatePlanTheme(theme);
    const definitionPath = join(
      repositoryRoot,
      "assets/theme-sources",
      theme.id,
      "theme-source.json",
    );
    const definition = definitionFromPlan(theme);
    writeFileSync(definitionPath, `${JSON.stringify(definition, null, 2)}\n`);
    const result = spawnSync(
      process.execPath,
      [
        builder,
        "--definition",
        definitionPath,
        "--output-root",
        outputRoot,
        "--catalog-template",
        catalogTemplate,
      ],
      { cwd: repositoryRoot, encoding: "utf8" },
    );
    if (result.status !== 0) {
      process.stderr.write(result.stderr);
      process.exitCode = 1;
      break;
    }
    catalogTemplate = join(outputRoot, "shared/theme-catalog.json");
  }

  if (process.exitCode !== 1) {
    removeStaleGeneratedPacks(new Set(plan.themes.map((theme) => theme.id)));
    removeRetiredBundledAssets();
    process.stdout.write(`${plan.themes.length} bundled themes built\n`);
  }
} finally {
  rmSync(temporaryDirectory, { recursive: true, force: true });
}

function definitionFromPlan(theme) {
  return {
    schema_version: 1,
    id: theme.id,
    name: theme.name,
    description: theme.description,
    category: "original-character",
    marketplace: theme.marketplace,
    source: {
      file: "source.png",
      original_file_name: theme.source_file,
      mime_type: "image/png",
      sha256: theme.sha256,
      width: theme.width,
      height: theme.height,
      bytes: theme.bytes,
    },
    runtime: {
      path: `src-tauri/resources/themes/${theme.id}.webp`,
      max_width: theme.width,
      max_height: theme.height,
      quality: 88,
      max_bytes: 1450000,
    },
    preview: {
      path: `public/themes/${theme.id}.webp`,
      max_width: theme.layout === "portrait" ? 405 : 720,
      max_height: theme.layout === "portrait" ? 720 : 405,
      quality: 78,
      max_bytes: 250000,
    },
    pack: {
      minimum_engine_version: 1,
      preview_path: `/themes/${theme.id}.webp`,
      overlay: theme.overlay,
      focal_x: theme.focal_x,
      focal_y: theme.focal_y,
      palette: theme.palette,
      effects: theme.effects,
    },
    rights: {
      source: `User-approved source: ${theme.source_file}`,
      rightsholder: "Codex Assistant asset contributor",
      license: "Redistribution permission confirmed for the Codex Assistant installer",
      commercial_redistribution: true,
      attribution:
        "Image supplied and redistribution approved by the Codex Assistant asset contributor",
      reviewed_at: "2026-07-30",
      manual_signoff: true,
      status: "verified",
    },
  };
}

function validatePlanTheme(theme) {
  if (
    typeof theme?.id !== "string" ||
    !/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(theme.id) ||
    typeof theme.name !== "string" ||
    typeof theme.source_file !== "string" ||
    !/^[a-f0-9]{64}$/.test(theme.sha256) ||
    !Number.isSafeInteger(theme.bytes) ||
    !Number.isSafeInteger(theme.width) ||
    !Number.isSafeInteger(theme.height) ||
    !["landscape", "portrait"].includes(theme.layout) ||
    !isValidThemeMarketplace(theme.marketplace)
  ) {
    throw new Error(`invalid approved theme plan entry: ${theme?.id ?? "unknown"}`);
  }
}

function removeStaleGeneratedPacks(approvedIds) {
  const directory = join(outputRoot, "shared/generated-theme-packs");
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    if (!entry.isFile() || !entry.name.endsWith(".json")) continue;
    const id = entry.name.slice(0, -5);
    if (!approvedIds.has(id)) {
      rmSync(join(directory, entry.name));
    }
  }
}

function removeRetiredBundledAssets() {
  const retired = [
    "public/themes/aurora-grid.svg",
    "public/themes/aurora-grid.webp",
    "public/themes/blush-circuit.webp",
    "public/themes/crimson-relay.webp",
    "public/themes/crystal-daylight.webp",
    "public/themes/cyan-chorus.webp",
    "public/themes/fortune-foundry.webp",
    "public/themes/gothic-horizon.webp",
    "public/themes/noir-stage.webp",
    "public/themes/original-observatory-muse.jpg",
    "public/themes/pocket-cosmos.webp",
    "public/themes/roseglass-atelier.webp",
    "public/themes/spring-street.webp",
    "public/themes/violet-blade.webp",
    "public/themes/violet-afterdark.webp",
    "src-tauri/resources/themes/original-observatory-muse.jpg",
    "src-tauri/resources/themes/spring-street.webp",
    "src-tauri/resources/themes/violet-blade.webp",
    "assets/theme-sources/spring-street",
    "assets/theme-sources/violet-blade",
  ];
  for (const ownedPath of retired) {
    if (isAbsolute(ownedPath)) throw new Error("retired theme path must be repository-relative");
    const target = resolve(outputRoot, ownedPath);
    if (relative(outputRoot, target).startsWith("..")) {
      throw new Error("retired theme path escaped the repository");
    }
    rmSync(target, { recursive: true, force: true });
  }
}
