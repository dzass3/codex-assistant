import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const script = path.join(root, "scripts", "qa", "mock-theme-contract.mjs");

for (let run = 1; run <= 3; run += 1) {
  const result = spawnSync(process.execPath, [script], {
    cwd: root,
    encoding: "utf8",
    maxBuffer: 128 * 1024 * 1024,
  });
  assert.equal(result.status, 0, `mock safety matrix run ${run} failed\n${result.stderr}`);
}

for (const canary of ["overlay", "semantic-recolor", "invisible-focus"]) {
  const result = spawnSync(process.execPath, [script], {
    cwd: root,
    encoding: "utf8",
    env: { ...process.env, MOCK_THEME_CANARY: canary },
    maxBuffer: 128 * 1024 * 1024,
  });
  assert.notEqual(result.status, 0, `${canary} canary unexpectedly passed`);
}

process.stdout.write(
  `${JSON.stringify({ status: "passed", cleanRuns: 3, rejectedCanaries: 3 })}\n`,
);
