import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { dirname, isAbsolute, relative, resolve } from "node:path";
import { isDeepStrictEqual } from "node:util";
import { chromium } from "playwright";
import { isValidThemeMarketplace } from "./theme-marketplace-validation.mjs";

const GENERATOR = "codex-assistant-theme-assets-v1";

main().catch((error) => {
  const message = error instanceof Error ? error.message : "theme asset build failed";
  process.stderr.write(`${message}\n`);
  process.exitCode = 1;
});

async function main() {
  const options = parseArguments(process.argv.slice(2));
  const definitionPath = realpathSync(options.definition);
  const definitionDirectory = dirname(definitionPath);
  const definition = JSON.parse(readFileSync(definitionPath, "utf8"));
  validateDefinition(definition);
  const outputRoot = resolve(options.outputRoot);
  const runtimePath = outputPath(outputRoot, definition.runtime.path);
  const previewPath = outputPath(outputRoot, definition.preview.path);
  const packPath = outputPath(outputRoot, `shared/generated-theme-packs/${definition.id}.json`);
  const manifestPath = outputPath(
    outputRoot,
    `assets/theme-sources/${definition.id}/build-manifest.json`,
  );
  const catalogPath = outputPath(outputRoot, "shared/theme-catalog.json");

  if (options.assembleVerifiedCatalog) {
    assembleCatalog({
      catalogTemplatePath: options.catalogTemplate,
      catalogPath,
      definition,
      manifestPath,
      outputRoot,
      packPath,
    });
    process.stdout.write(`${definition.id}\n`);
    return;
  }

  const sourcePath = confinedPath(definitionDirectory, definition.source.file, "source image");
  const sourceBytes = readFileSync(sourcePath);
  const sourceHash = sha256(sourceBytes);
  if (sourceHash !== definition.source.sha256.toLowerCase()) {
    throw new Error("source image hash does not match the approved definition");
  }
  if (sourceBytes.byteLength !== definition.source.bytes) {
    throw new Error("source image byte length does not match the approved definition");
  }
  if (!hasPngSignature(sourceBytes) || definition.source.mime_type !== "image/png") {
    throw new Error("source image MIME does not match the approved definition");
  }

  const browser = await chromium.launch({ headless: true });
  let runtime;
  let preview;
  let adaptation;
  try {
    const page = await browser.newPage();
    const dataUrl = `data:image/png;base64,${sourceBytes.toString("base64")}`;
    adaptation = await analyzeBackdrop(page, dataUrl);
    runtime = await encodeWebp(page, dataUrl, definition.runtime);
    preview = await encodeWebp(page, dataUrl, definition.preview);
  } finally {
    await browser.close();
  }

  if (
    runtime.source_width !== definition.source.width ||
    runtime.source_height !== definition.source.height
  ) {
    throw new Error("source image dimensions do not match the approved definition");
  }
  enforceDerivedBudget(runtime, definition.runtime, "runtime");
  enforceDerivedBudget(preview, definition.preview, "preview");

  writeBytes(runtimePath, runtime.bytes);
  writeBytes(previewPath, preview.bytes);

  const pack = expectedPack(definition, sha256(runtime.bytes), adaptation);
  writeJson(packPath, pack);

  const manifest = {
    schema_version: 1,
    generator: GENERATOR,
    theme_id: definition.id,
    source: {
      file: definition.source.file,
      original_file_name: definition.source.original_file_name,
      mime_type: definition.source.mime_type,
      sha256: sourceHash,
      width: definition.source.width,
      height: definition.source.height,
      bytes: sourceBytes.byteLength,
    },
    runtime: derivedRecord(definition.runtime, runtime),
    preview: derivedRecord(definition.preview, preview),
    adaptation,
    rights: definition.rights,
  };
  writeJson(manifestPath, manifest);
  assembleCatalog({
    catalogTemplatePath: options.catalogTemplate,
    catalogPath,
    definition,
    manifestPath,
    outputRoot,
    packPath,
  });
  process.stdout.write(`${definition.id}\n`);
}

function parseArguments(argumentsList) {
  const values = new Map();
  let assembleVerifiedCatalog = false;
  for (let index = 0; index < argumentsList.length;) {
    const name = argumentsList[index];
    if (name === "--assemble-verified-catalog") {
      assembleVerifiedCatalog = true;
      index += 1;
      continue;
    }
    const value = argumentsList[index + 1];
    if (
      !["--definition", "--output-root", "--catalog-template"].includes(name) ||
      !value ||
      values.has(name)
    ) {
      throw new Error(
        "usage: --definition <file> --output-root <directory> --catalog-template <file> [--assemble-verified-catalog]",
      );
    }
    values.set(name, value);
    index += 2;
  }
  const definition = values.get("--definition");
  const outputRoot = values.get("--output-root");
  const catalogTemplate = values.get("--catalog-template");
  if (!definition || !outputRoot || !catalogTemplate || values.size !== 3) {
    throw new Error(
      "usage: --definition <file> --output-root <directory> --catalog-template <file> [--assemble-verified-catalog]",
    );
  }
  return {
    definition: resolve(definition),
    outputRoot,
    catalogTemplate: resolve(catalogTemplate),
    assembleVerifiedCatalog,
  };
}

function validateDefinition(definition) {
  if (
    definition?.schema_version !== 1 ||
    !safeSlug(definition.id) ||
    !nonEmptyString(definition.name) ||
    !nonEmptyString(definition.description) ||
    !["abstract", "original-character", "project-showcase"].includes(definition.category) ||
    definition.source?.file !== "source.png" ||
    !nonEmptyString(definition.source?.original_file_name) ||
    typeof definition.source?.sha256 !== "string" ||
    !/^[a-f0-9]{64}$/i.test(definition.source.sha256) ||
    !Number.isSafeInteger(definition.source?.bytes) ||
    !Number.isSafeInteger(definition.source?.width) ||
    !Number.isSafeInteger(definition.source?.height) ||
    definition.source.bytes <= 0 ||
    definition.source.width <= 0 ||
    definition.source.height <= 0 ||
    definition.runtime?.path !== `src-tauri/resources/themes/${definition.id}.webp` ||
    definition.preview?.path !== `public/themes/${definition.id}.webp` ||
    definition.pack?.preview_path !== `/themes/${definition.id}.webp` ||
    !isValidThemeMarketplace(definition.marketplace)
  ) {
    throw new Error("theme source definition is invalid");
  }
  if (!approvedRights(definition.rights)) {
    throw new Error("theme source rights are not approved for redistribution");
  }
  for (const variant of [definition.runtime, definition.preview]) {
    if (
      typeof variant?.path !== "string" ||
      !Number.isInteger(variant.max_width) ||
      !Number.isInteger(variant.max_height) ||
      !Number.isInteger(variant.quality) ||
      !Number.isInteger(variant.max_bytes) ||
      variant.max_width <= 0 ||
      variant.max_height <= 0 ||
      variant.quality < 1 ||
      variant.quality > 100 ||
      variant.max_bytes <= 0
    ) {
      throw new Error("theme source derivation is invalid");
    }
  }
}

async function encodeWebp(page, source, variant) {
  const encoded = await page.evaluate(
    async ({ sourceUrl, maxWidth, maxHeight, quality }) => {
      const image = new Image();
      image.src = sourceUrl;
      await image.decode();
      const scale = Math.min(1, maxWidth / image.naturalWidth, maxHeight / image.naturalHeight);
      const width = Math.max(1, Math.round(image.naturalWidth * scale));
      const height = Math.max(1, Math.round(image.naturalHeight * scale));
      const canvas = document.createElement("canvas");
      canvas.width = width;
      canvas.height = height;
      const context = canvas.getContext("2d", { alpha: false });
      if (!context) throw new Error("image encoder is unavailable");
      context.drawImage(image, 0, 0, width, height);
      return {
        data_url: canvas.toDataURL("image/webp", quality / 100),
        width,
        height,
        source_width: image.naturalWidth,
        source_height: image.naturalHeight,
      };
    },
    {
      sourceUrl: source,
      maxWidth: variant.max_width,
      maxHeight: variant.max_height,
      quality: variant.quality,
    },
  );
  const prefix = "data:image/webp;base64,";
  if (!encoded.data_url.startsWith(prefix)) {
    throw new Error("image encoder did not produce WebP");
  }
  return {
    bytes: Buffer.from(encoded.data_url.slice(prefix.length), "base64"),
    width: encoded.width,
    height: encoded.height,
    source_width: encoded.source_width,
    source_height: encoded.source_height,
  };
}

async function analyzeBackdrop(page, source) {
  return page.evaluate(async (sourceUrl) => {
    const image = new Image();
    image.src = sourceUrl;
    await image.decode();
    const size = 64;
    const canvas = document.createElement("canvas");
    canvas.width = size;
    canvas.height = size;
    const context = canvas.getContext("2d", {
      alpha: false,
      colorSpace: "srgb",
      willReadFrequently: true,
    });
    if (!context) throw new Error("image analyzer is unavailable");
    context.drawImage(image, 0, 0, size, size);
    const pixels = context.getImageData(0, 0, size, size).data;
    const luminance = new Float64Array(size * size);
    let luminanceTotal = 0;
    let saturationTotal = 0;
    for (let index = 0; index < luminance.length; index += 1) {
      const offset = index * 4;
      const red = pixels[offset] / 255;
      const green = pixels[offset + 1] / 255;
      const blue = pixels[offset + 2] / 255;
      const maximum = Math.max(red, green, blue);
      const minimum = Math.min(red, green, blue);
      const value = 0.2126 * red + 0.7152 * green + 0.0722 * blue;
      luminance[index] = value;
      luminanceTotal += value;
      saturationTotal += maximum === 0 ? 0 : (maximum - minimum) / maximum;
    }
    const average = luminanceTotal / luminance.length;
    let varianceTotal = 0;
    let edgeTotal = 0;
    let edgeCount = 0;
    for (let y = 0; y < size; y += 1) {
      for (let x = 0; x < size; x += 1) {
        const index = y * size + x;
        const value = luminance[index];
        varianceTotal += (value - average) ** 2;
        if (x + 1 < size) {
          edgeTotal += Math.abs(value - luminance[index + 1]);
          edgeCount += 1;
        }
        if (y + 1 < size) {
          edgeTotal += Math.abs(value - luminance[index + size]);
          edgeCount += 1;
        }
      }
    }
    const deviation = Math.sqrt(varianceTotal / luminance.length);
    const edgeDensity = edgeCount === 0 ? 0 : edgeTotal / edgeCount;
    return {
      luminance: Math.max(0, Math.min(100, Math.round(average * 100))),
      complexity: Math.max(0, Math.min(100, Math.round(edgeDensity * 260 + deviation * 95))),
      saturation: Math.max(
        0,
        Math.min(100, Math.round((saturationTotal / luminance.length) * 100)),
      ),
    };
  }, source);
}

function enforceDerivedBudget(derived, variant, label) {
  if (derived.bytes.byteLength === 0 || derived.bytes.byteLength > variant.max_bytes) {
    throw new Error(`${label} WebP exceeds the approved byte budget`);
  }
  if (
    derived.width <= 0 ||
    derived.height <= 0 ||
    derived.width > variant.max_width ||
    derived.height > variant.max_height
  ) {
    throw new Error(`${label} WebP exceeds the approved dimension budget`);
  }
}

function assembleCatalog({
  catalogTemplatePath,
  catalogPath,
  definition,
  manifestPath,
  outputRoot,
  packPath,
}) {
  const pack = readJson(packPath, "generated theme pack");
  const manifest = readJson(manifestPath, "theme build manifest");
  validateCatalogCandidate({ definition, manifest, outputRoot, pack });

  const catalog = readJson(catalogTemplatePath, "theme catalog template");
  if (
    catalog?.schema_version !== 1 ||
    !Array.isArray(catalog.themes) ||
    catalog.themes.some((theme) => !safeSlug(theme?.id))
  ) {
    throw new Error("theme catalog template is invalid");
  }
  const themes = catalog.themes.filter((theme) => theme.id !== definition.id);
  writeJson(catalogPath, { ...catalog, themes: [...themes, pack] });
}

function validateCatalogCandidate({ definition, manifest, outputRoot, pack }) {
  if (!approvedRights(pack?.rights) || !isDeepStrictEqual(pack.rights, definition.rights)) {
    throw new Error("generated theme pack rights are not eligible for catalog assembly");
  }
  if (
    manifest?.schema_version !== 1 ||
    manifest.generator !== GENERATOR ||
    manifest.theme_id !== definition.id ||
    !isDeepStrictEqual(manifest.rights, definition.rights)
  ) {
    throw new Error("theme build manifest is not eligible for catalog assembly");
  }

  validateDerivedRecord(manifest.runtime, definition.runtime, "runtime");
  validateDerivedRecord(manifest.preview, definition.preview, "preview");
  verifyDerivedFile(outputRoot, manifest.runtime, "runtime");
  verifyDerivedFile(outputRoot, manifest.preview, "preview");

  if (
    !validAdaptation(manifest.adaptation) ||
    !isDeepStrictEqual(pack, expectedPack(definition, manifest.runtime.sha256, manifest.adaptation))
  ) {
    throw new Error("generated theme pack metadata is not eligible for catalog assembly");
  }
}

function validateDerivedRecord(record, variant, label) {
  if (
    record?.path !== variant.path ||
    record.mime_type !== "image/webp" ||
    typeof record.sha256 !== "string" ||
    !/^[a-f0-9]{64}$/i.test(record.sha256)
  ) {
    throw new Error(`${label} catalog hash or MIME evidence is missing`);
  }
  if (
    !Number.isSafeInteger(record.width) ||
    !Number.isSafeInteger(record.height) ||
    !Number.isSafeInteger(record.bytes) ||
    record.width <= 0 ||
    record.height <= 0 ||
    record.width > variant.max_width ||
    record.height > variant.max_height ||
    record.bytes <= 0 ||
    record.bytes > variant.max_bytes
  ) {
    throw new Error(`${label} catalog dimension or byte evidence is invalid`);
  }
}

function verifyDerivedFile(outputRoot, record, label) {
  const path = outputPath(outputRoot, record.path);
  let bytes;
  try {
    bytes = readFileSync(path);
  } catch {
    throw new Error(`${label} catalog resource is missing`);
  }
  if (!hasWebpSignature(bytes)) {
    throw new Error(`${label} catalog resource MIME is invalid`);
  }
  if (bytes.byteLength !== record.bytes || sha256(bytes) !== record.sha256) {
    throw new Error(`${label} catalog resource hash or byte evidence does not match`);
  }
  const dimensions = webpDimensions(bytes);
  if (dimensions.width !== record.width || dimensions.height !== record.height) {
    throw new Error(`${label} catalog resource dimensions do not match the manifest`);
  }
}

function readJson(path, label) {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch {
    throw new Error(`${label} is missing or invalid`);
  }
}

function derivedRecord(variant, derived) {
  return {
    path: variant.path,
    mime_type: "image/webp",
    sha256: sha256(derived.bytes),
    width: derived.width,
    height: derived.height,
    bytes: derived.bytes.byteLength,
    quality: variant.quality,
  };
}

function expectedPack(definition, runtimeHash, adaptation) {
  return {
    schema_version: 1,
    minimum_engine_version: definition.pack.minimum_engine_version,
    id: definition.id,
    name: definition.name,
    description: definition.description,
    category: definition.category,
    marketplace: definition.marketplace,
    preview_path: definition.pack.preview_path,
    backdrop: {
      kind: "image",
      asset_id: definition.id,
      overlay: definition.pack.overlay,
      focal_x: definition.pack.focal_x,
      focal_y: definition.pack.focal_y,
    },
    palette: definition.pack.palette,
    effects: definition.pack.effects,
    adaptation,
    assets: [
      {
        id: definition.id,
        mime_type: "image/webp",
        sha256: runtimeHash,
      },
    ],
    rights: definition.rights,
  };
}

function validAdaptation(value) {
  return (
    value !== null &&
    typeof value === "object" &&
    ["luminance", "complexity", "saturation"].every(
      (key) => Number.isSafeInteger(value[key]) && value[key] >= 0 && value[key] <= 100,
    )
  );
}

function confinedPath(root, child, label) {
  if (typeof child !== "string" || child.length === 0 || isAbsolute(child)) {
    throw new Error(`${label} path is invalid`);
  }
  const candidate = resolve(root, child);
  if (relative(root, candidate).startsWith("..")) {
    throw new Error(`${label} path escapes its owned directory`);
  }
  return realpathSync(candidate);
}

function outputPath(outputRoot, child) {
  if (typeof child !== "string" || child.length === 0 || isAbsolute(child)) {
    throw new Error("theme output path is invalid");
  }
  const candidate = resolve(outputRoot, child);
  if (relative(outputRoot, candidate).startsWith("..")) {
    throw new Error("theme output path escapes its owned directory");
  }
  return candidate;
}

function writeBytes(path, bytes) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, bytes);
}

function writeJson(path, value) {
  writeBytes(path, Buffer.from(`${JSON.stringify(value, null, 2)}\n`, "utf8"));
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function hasPngSignature(bytes) {
  return (
    bytes.length >= 8 &&
    bytes[0] === 0x89 &&
    bytes[1] === 0x50 &&
    bytes[2] === 0x4e &&
    bytes[3] === 0x47 &&
    bytes[4] === 0x0d &&
    bytes[5] === 0x0a &&
    bytes[6] === 0x1a &&
    bytes[7] === 0x0a
  );
}

function hasWebpSignature(bytes) {
  return (
    bytes.length >= 12 &&
    bytes.subarray(0, 4).toString("ascii") === "RIFF" &&
    bytes.subarray(8, 12).toString("ascii") === "WEBP"
  );
}

function webpDimensions(bytes) {
  if (!hasWebpSignature(bytes)) {
    throw new Error("WebP signature is invalid");
  }
  let offset = 12;
  while (offset + 8 <= bytes.length) {
    const chunkType = bytes.subarray(offset, offset + 4).toString("ascii");
    const chunkSize = bytes.readUInt32LE(offset + 4);
    const payload = offset + 8;
    if (payload + chunkSize > bytes.length) {
      throw new Error("WebP chunk is truncated");
    }
    if (chunkType === "VP8X" && chunkSize >= 10) {
      return {
        width: bytes.readUIntLE(payload + 4, 3) + 1,
        height: bytes.readUIntLE(payload + 7, 3) + 1,
      };
    }
    if (chunkType === "VP8L" && chunkSize >= 5 && bytes[payload] === 0x2f) {
      const packed = bytes.readUInt32LE(payload + 1);
      return {
        width: (packed & 0x3fff) + 1,
        height: ((packed >>> 14) & 0x3fff) + 1,
      };
    }
    if (
      chunkType === "VP8 " &&
      chunkSize >= 10 &&
      bytes[payload + 3] === 0x9d &&
      bytes[payload + 4] === 0x01 &&
      bytes[payload + 5] === 0x2a
    ) {
      return {
        width: bytes.readUInt16LE(payload + 6) & 0x3fff,
        height: bytes.readUInt16LE(payload + 8) & 0x3fff,
      };
    }
    offset = payload + chunkSize + (chunkSize % 2);
  }
  throw new Error("WebP dimensions are unavailable");
}

function approvedRights(rights) {
  return (
    rights?.status === "verified" &&
    rights?.commercial_redistribution === true &&
    rights?.manual_signoff === true &&
    nonEmptyString(rights?.source) &&
    nonEmptyString(rights?.rightsholder) &&
    nonEmptyString(rights?.license) &&
    nonEmptyString(rights?.attribution) &&
    /^\d{4}-\d{2}-\d{2}$/.test(rights?.reviewed_at)
  );
}

function nonEmptyString(value) {
  return typeof value === "string" && value.trim().length > 0;
}

function safeSlug(value) {
  return (
    typeof value === "string" &&
    value.length > 0 &&
    value.length <= 80 &&
    /^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(value)
  );
}
