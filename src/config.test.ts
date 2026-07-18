import { describe, expect, it } from "vitest";
import { DEFAULT_REFRESH_MS, MONITOR_EVENT, PRODUCT_NAME } from "./config";

describe("product configuration", () => {
  it("uses the monitor identity and event contract", () => {
    expect(PRODUCT_NAME).toBe("Codex Agent Monitor");
    expect(MONITOR_EVENT).toBe("monitor://snapshot");
    expect(DEFAULT_REFRESH_MS).toBe(1000);
  });
});
