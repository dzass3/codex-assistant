import { describe, expect, it } from "vitest";
import {
  DEFAULT_REFRESH_MS,
  MONITOR_EVENT,
  PRODUCT_NAME,
  PRODUCT_TAGLINE,
} from "./config";

describe("product configuration", () => {
  it("uses the Codex Assistant identity and monitor event contract", () => {
    expect(PRODUCT_NAME).toBe("Codex Assistant");
    expect(PRODUCT_TAGLINE).toBe("原生代理路由、模型观察与主题管理");
    expect(MONITOR_EVENT).toBe("monitor://snapshot");
    expect(DEFAULT_REFRESH_MS).toBe(1000);
  });
});
