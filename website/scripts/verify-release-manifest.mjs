import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const releaseDataPath = new URL("../app/release-data.ts", import.meta.url);
const source = await readFile(releaseDataPath, "utf8");

assert.match(source, /releaseVersion = "0\.12\.0"/);
assert.equal((source.match(/fileName:/g) ?? []).length, 4);
assert.equal((source.match(/sha256: "[A-F0-9]{64}"/g) ?? []).length, 4);
assert.match(source, /releases\/download\/v0\.12\.0/);
assert.doesNotMatch(source, /website\/public\/downloads|0\.11\.7|0\.11\.8|0\.11\.9/);

process.stdout.write("Release manifest: 0.12.0, four external assets, SHA-256 present.\n");
