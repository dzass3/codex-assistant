import { createHash } from "node:crypto";
import {
  copyFileSync,
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import { afterEach, describe, expect, it } from "vitest";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const command = join(repositoryRoot, "scripts/themes/build-bundled-theme.mjs");
const definition = join(repositoryRoot, "assets/theme-sources/wisteria-bride/theme-source.json");
const temporaryRoots: string[] = [];

interface ThemeSourceFixture {
  source: {
    sha256: string;
    mime_type: string;
  };
  runtime: {
    max_width: number;
    max_height: number;
    max_bytes: number;
  };
}

afterEach(() => {
  while (temporaryRoots.length > 0) {
    const root = temporaryRoots.pop();
    if (root) rmSync(root, { recursive: true, force: true });
  }
});

describe("bundled theme asset command", () => {
  it("builds reproducible, rights-gated offline artifacts for Wisteria Bride", () => {
    const first = temporaryRoot();
    const second = temporaryRoot();

    expect(runBuild(first)).toMatchObject({ status: 0, stderr: "" });
    expect(runBuild(second)).toMatchObject({ status: 0, stderr: "" });

    const relativeFiles = [
      "src-tauri/resources/themes/wisteria-bride.webp",
      "public/themes/wisteria-bride.webp",
      "shared/generated-theme-packs/wisteria-bride.json",
      "assets/theme-sources/wisteria-bride/build-manifest.json",
    ];
    for (const relativeFile of relativeFiles) {
      expect(existsSync(join(first, relativeFile))).toBe(true);
      expect(readFileSync(join(first, relativeFile))).toEqual(
        readFileSync(join(second, relativeFile)),
      );
    }

    const runtime = readFileSync(join(first, "src-tauri/resources/themes/wisteria-bride.webp"));
    const preview = readFileSync(join(first, "public/themes/wisteria-bride.webp"));
    expect(runtime.subarray(0, 4).toString("ascii")).toBe("RIFF");
    expect(runtime.subarray(8, 12).toString("ascii")).toBe("WEBP");
    expect(preview.subarray(0, 4).toString("ascii")).toBe("RIFF");
    expect(preview.subarray(8, 12).toString("ascii")).toBe("WEBP");
    expect(runtime.includes(Buffer.from("EXIF"))).toBe(false);
    expect(preview.includes(Buffer.from("EXIF"))).toBe(false);

    const manifest = JSON.parse(
      readFileSync(join(first, "assets/theme-sources/wisteria-bride/build-manifest.json"), "utf8"),
    );
    expect(manifest).toMatchObject({
      schema_version: 1,
      generator: "codex-assistant-theme-assets-v1",
      theme_id: "wisteria-bride",
      source: {
        file: "source.png",
        original_file_name: "ChatGPT Image 2026年7月24日 11_20_32 (3).png",
        mime_type: "image/png",
        sha256: "33a66e9b2024de2cee165c7fb822fcf3bf54cde0f599629634f28dc4c1645cb9",
        width: 1672,
        height: 941,
        bytes: 2019217,
      },
      rights: {
        rightsholder: "Codex Assistant asset contributor",
        commercial_redistribution: true,
        manual_signoff: true,
        status: "verified",
      },
    });
    expect(manifest.runtime.sha256).toBe(sha256(runtime));
    expect(manifest.runtime.mime_type).toBe("image/webp");
    expect(manifest.runtime.bytes).toBe(runtime.byteLength);
    expect(manifest.preview.sha256).toBe(sha256(preview));
    expect(manifest.preview.mime_type).toBe("image/webp");
    expect(manifest.preview.bytes).toBe(preview.byteLength);

    const pack = JSON.parse(
      readFileSync(join(first, "shared/generated-theme-packs/wisteria-bride.json"), "utf8"),
    );
    expect(pack).toMatchObject({
      schema_version: 1,
      minimum_engine_version: 1,
      id: "wisteria-bride",
      name: "紫藤花嫁",
      preview_path: "/themes/wisteria-bride.webp",
      backdrop: { kind: "image", asset_id: "wisteria-bride" },
      assets: [
        {
          id: "wisteria-bride",
          mime_type: "image/webp",
          sha256: sha256(runtime),
        },
      ],
      rights: manifest.rights,
    });
  });

  it("rejects an incomplete redistribution rights record", () => {
    const fixture = temporaryRoot();
    const sourceDefinition = JSON.parse(readFileSync(definition, "utf8"));
    delete sourceDefinition.rights.license;
    const invalidDefinition = join(fixture, "theme-source.json");
    copyFileSync(
      join(repositoryRoot, "assets/theme-sources/wisteria-bride/source.png"),
      join(fixture, "source.png"),
    );
    writeFileSync(invalidDefinition, `${JSON.stringify(sourceDefinition, null, 2)}\n`);

    const result = runBuild(join(fixture, "output"), invalidDefinition);

    expect(result.status).toBe(1);
    expect(result.stderr).toMatch(/rights/i);
  });

  it("builds another valid stable theme definition without command changes", () => {
    const fixture = temporaryRoot();
    const sourceDefinition = JSON.parse(readFileSync(definition, "utf8"));
    sourceDefinition.id = "catalog-canary";
    sourceDefinition.name = "目录样板";
    sourceDefinition.runtime.path = "src-tauri/resources/themes/catalog-canary.webp";
    sourceDefinition.preview.path = "public/themes/catalog-canary.webp";
    sourceDefinition.pack.preview_path = "/themes/catalog-canary.webp";
    const canaryDefinition = join(fixture, "theme-source.json");
    copyFileSync(
      join(repositoryRoot, "assets/theme-sources/wisteria-bride/source.png"),
      join(fixture, "source.png"),
    );
    writeFileSync(canaryDefinition, `${JSON.stringify(sourceDefinition, null, 2)}\n`);
    const output = join(fixture, "output");

    const result = runBuild(output, canaryDefinition);

    expect(result).toMatchObject({ status: 0, stderr: "" });
    const pack = JSON.parse(
      readFileSync(join(output, "shared/generated-theme-packs/catalog-canary.json"), "utf8"),
    );
    expect(pack).toMatchObject({
      id: "catalog-canary",
      name: "目录样板",
      preview_path: "/themes/catalog-canary.webp",
      backdrop: { asset_id: "catalog-canary" },
      assets: [{ id: "catalog-canary" }],
    });
  });

  it("rejects a source whose bytes do not match the approved hash", () => {
    const fixture = definitionFixture((value) => {
      value.source.sha256 = "0".repeat(64);
    });

    const result = runBuild(join(fixture.root, "output"), fixture.definition);

    expect(result.status).toBe(1);
    expect(result.stderr).toMatch(/hash/i);
  });

  it("rejects a source whose declared MIME does not match its signature", () => {
    const fixture = definitionFixture((value) => {
      value.source.mime_type = "image/jpeg";
    });

    const result = runBuild(join(fixture.root, "output"), fixture.definition);

    expect(result.status).toBe(1);
    expect(result.stderr).toMatch(/MIME/i);
  });

  it("rejects a derived asset that exceeds its declared byte budget", () => {
    const fixture = definitionFixture((value) => {
      value.runtime.max_bytes = 1;
    });

    const result = runBuild(join(fixture.root, "output"), fixture.definition);

    expect(result.status).toBe(1);
    expect(result.stderr).toMatch(/budget/i);
  });

  it("rejects invalid or exceeded derived image dimensions", () => {
    const invalidLimit = definitionFixture((value) => {
      value.runtime.max_width = 0;
    });

    const invalidResult = runBuild(join(invalidLimit.root, "output"), invalidLimit.definition);

    expect(invalidResult.status).toBe(1);
    expect(invalidResult.stderr).toMatch(/derivation|dimension/i);
  });

  it.each(["resource", "rights", "hash"] as const)(
    "keeps the theme out of the assembled catalog when its %s evidence is missing",
    (missingEvidence) => {
      const fixture = temporaryRoot();
      const output = join(fixture, "output");
      const catalogTemplate = catalogWithoutWisteria(fixture);
      expect(runBuild(output, definition, catalogTemplate)).toMatchObject({
        status: 0,
        stderr: "",
      });
      rmSync(join(output, "shared/theme-catalog.json"));

      if (missingEvidence === "resource") {
        unlinkSync(join(output, "src-tauri/resources/themes/wisteria-bride.webp"));
      } else if (missingEvidence === "rights") {
        const packPath = join(output, "shared/generated-theme-packs/wisteria-bride.json");
        const pack = JSON.parse(readFileSync(packPath, "utf8"));
        delete pack.rights.license;
        writeFileSync(packPath, `${JSON.stringify(pack, null, 2)}\n`);
      } else {
        const manifestPath = join(
          output,
          "assets/theme-sources/wisteria-bride/build-manifest.json",
        );
        const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
        delete manifest.runtime.sha256;
        writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
      }

      const result = runBuild(output, definition, catalogTemplate, true);

      expect(result.status).toBe(1);
      expect(result.stderr).not.toMatch(/usage/i);
      expect(result.stderr).toMatch(/catalog|resource|rights|hash/i);
      expect(existsSync(join(output, "shared/theme-catalog.json"))).toBe(false);
    },
  );

  it("rejects a generated pack whose declarative theme metadata was changed", () => {
    const fixture = temporaryRoot();
    const output = join(fixture, "output");
    const catalogTemplate = catalogWithoutWisteria(fixture);
    expect(runBuild(output, definition, catalogTemplate)).toMatchObject({
      status: 0,
      stderr: "",
    });
    rmSync(join(output, "shared/theme-catalog.json"));
    const packPath = join(output, "shared/generated-theme-packs/wisteria-bride.json");
    const pack = JSON.parse(readFileSync(packPath, "utf8"));
    pack.effects.blur_px = 1;
    writeFileSync(packPath, `${JSON.stringify(pack, null, 2)}\n`);

    const result = runBuild(output, definition, catalogTemplate, true);

    expect(result.status).toBe(1);
    expect(result.stderr).not.toMatch(/usage/i);
    expect(result.stderr).toMatch(/pack|metadata|catalog/i);
    expect(existsSync(join(output, "shared/theme-catalog.json"))).toBe(false);
  });

  it("rejects a manifest whose dimensions do not match the actual WebP", () => {
    const fixture = temporaryRoot();
    const output = join(fixture, "output");
    const catalogTemplate = catalogWithoutWisteria(fixture);
    expect(runBuild(output, definition, catalogTemplate)).toMatchObject({
      status: 0,
      stderr: "",
    });
    rmSync(join(output, "shared/theme-catalog.json"));
    const manifestPath = join(output, "assets/theme-sources/wisteria-bride/build-manifest.json");
    const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
    manifest.runtime.width = 1;
    manifest.runtime.height = 1;
    writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);

    const result = runBuild(output, definition, catalogTemplate, true);

    expect(result.status).toBe(1);
    expect(result.stderr).not.toMatch(/usage/i);
    expect(result.stderr).toMatch(/dimension/i);
    expect(existsSync(join(output, "shared/theme-catalog.json"))).toBe(false);
  });
});

function runBuild(
  outputRoot: string,
  definitionPath = definition,
  catalogTemplate = join(repositoryRoot, "shared/theme-catalog.json"),
  assembleVerifiedCatalog = false,
) {
  const argumentsList = [
    command,
    "--definition",
    definitionPath,
    "--output-root",
    outputRoot,
    "--catalog-template",
    catalogTemplate,
  ];
  if (assembleVerifiedCatalog) argumentsList.push("--assemble-verified-catalog");
  const result = spawnSync(process.execPath, argumentsList, {
    cwd: repositoryRoot,
    encoding: "utf8",
  });
  return {
    status: result.status,
    stderr: result.stderr.trim(),
  };
}

function catalogWithoutWisteria(root: string): string {
  const catalog = JSON.parse(
    readFileSync(join(repositoryRoot, "shared/theme-catalog.json"), "utf8"),
  );
  catalog.themes = catalog.themes.filter((theme: { id: string }) => theme.id !== "wisteria-bride");
  const path = join(root, "catalog-template.json");
  writeFileSync(path, `${JSON.stringify(catalog, null, 2)}\n`);
  return path;
}

function temporaryRoot(): string {
  const root = mkdtempSync(join(tmpdir(), "codex-assistant-theme-build-"));
  temporaryRoots.push(root);
  return root;
}

function sha256(bytes: Uint8Array): string {
  return createHash("sha256").update(bytes).digest("hex");
}

function definitionFixture(mutate: (definitionValue: ThemeSourceFixture) => void): {
  root: string;
  definition: string;
} {
  const root = temporaryRoot();
  const value = JSON.parse(readFileSync(definition, "utf8")) as ThemeSourceFixture;
  mutate(value);
  copyFileSync(
    join(repositoryRoot, "assets/theme-sources/wisteria-bride/source.png"),
    join(root, "source.png"),
  );
  const definitionPath = join(root, "theme-source.json");
  writeFileSync(definitionPath, `${JSON.stringify(value, null, 2)}\n`);
  return { root, definition: definitionPath };
}
