import { describe, expect, it } from "vitest";
import { toRoutingSnapshot } from "./routingApi";

const snapshot = {
  schema_version: 1,
  profile_version: "routing-v1",
  routes: [
    {
      route_key: "d2719d93-b823-4a7f-934f-23cbe01c8aaf",
      conversation_id: "d2719d93-b823-4a7f-934f-23cbe01c8ab0",
      enabled: true,
      phase: "implementing",
      created_at_ms: 1,
      updated_at_ms: 2,
    },
  ],
  eligibility: [
    {
      tier: "terra",
      route_kind: "direct",
      status: "eligible",
      checked_at_ms: 2,
      profile_version: "routing-v1",
    },
  ],
  activity: [
    {
      route_key: "d2719d93-b823-4a7f-934f-23cbe01c8aaf",
      child_thread_id: "d2719d93-b823-4a7f-934f-23cbe01c8ab1",
      subtask_id: "d2719d93-b823-4a7f-934f-23cbe01c8ab2",
      route_kind: "direct",
      phase: "implementing",
      is_reviewer: false,
      escalation_count: 0,
      started_at_ms: 2,
      updated_at_ms: 3,
    },
  ],
};

describe("routing snapshot boundary", () => {
  it("accepts only the explicit sanitized contract", () => {
    expect(toRoutingSnapshot(snapshot)).toEqual(snapshot);
  });

  it.each([
    (() => {
      const missingSchemaVersion: Record<string, unknown> = { ...snapshot };
      delete missingSchemaVersion.schema_version;
      return missingSchemaVersion;
    })(),
    { ...snapshot, routes: [{ ...snapshot.routes[0], phase: "mystery" }] },
    { ...snapshot, eligibility: [{ ...snapshot.eligibility[0], status: "mystery" }] },
    { ...snapshot, routes: [{ ...snapshot.routes[0], route_key: "not-a-uuid" }] },
    { ...snapshot, profile_version: "version with task content" },
    { ...snapshot, prompt: "CANARY_PRIVATE_PROMPT" },
    { ...snapshot, routes: [{ ...snapshot.routes[0], secret: "CANARY" }] },
  ])("fails closed for malformed or content-bearing payloads", (value) => {
    expect(toRoutingSnapshot(value)).toBeNull();
  });
});
