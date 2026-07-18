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
      codex_package_version: "1.2.3",
      requested_model: "gpt-5.6-terra",
      depth: 1,
      reason: null,
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
      parent_thread_id: "d2719d93-b823-4a7f-934f-23cbe01c8ab0",
      escalation_count: 0,
      selected_tier: "terra",
      requested_tier: "terra",
      effective_tier: "terra",
      reason_codes: ["cross-layer-work"],
      started_at_ms: 2,
      updated_at_ms: 3,
    },
  ],
  quality: [],
};

describe("routing snapshot boundary", () => {
  it("accepts only the explicit sanitized contract", () => {
    expect(toRoutingSnapshot(snapshot)).toEqual(snapshot);
  });

  it("keeps a historical reviewer attached to the implementation attempt it reviewed", () => {
    const initial = { ...snapshot.activity[0], phase: "completed" };
    const escalated = {
      ...initial,
      child_thread_id: "d2719d93-b823-4a7f-934f-23cbe01c8ab3",
      escalation_count: 1,
    };
    const reviewer = {
      ...initial,
      child_thread_id: "d2719d93-b823-4a7f-934f-23cbe01c8ab4",
      is_reviewer: true,
      escalation_count: 0,
    };
    const value = { ...snapshot, activity: [initial, escalated, reviewer] };
    expect(toRoutingSnapshot(value)).toEqual(value);
  });

  it("accepts only Terra-parented lower-tier nested work", () => {
    const nested = {
      ...snapshot.activity[0],
      child_thread_id: "d2719d93-b823-4a7f-934f-23cbe01c8ab6",
      subtask_id: "d2719d93-b823-4a7f-934f-23cbe01c8ab7",
      route_kind: "nested",
      parent_thread_id: snapshot.activity[0].child_thread_id,
      selected_tier: "luna",
      requested_tier: "luna",
      effective_tier: "luna",
    };
    const value = { ...snapshot, activity: [...snapshot.activity, nested] };
    expect(toRoutingSnapshot(value)).toEqual(value);
    expect(
      toRoutingSnapshot({
        ...value,
        activity: [{ ...snapshot.activity[0], selected_tier: "luna" }, nested],
      }),
    ).toBeNull();
    expect(
      toRoutingSnapshot({
        ...value,
        activity: [...snapshot.activity, { ...nested, selected_tier: "terra" }],
      }),
    ).toBeNull();
  });

  it("accepts quality metadata only when it matches the terminal child exactly", () => {
    const completed = { ...snapshot.activity[0], phase: "completed", updated_at_ms: 4 };
    const quality = {
      route_key: completed.route_key,
      child_thread_id: completed.child_thread_id,
      outcome: "passed",
      reviewer_tier: "sol",
      retry_count: 0,
      escalation_count: 0,
      recorded_at_ms: 4,
    };
    const value = { ...snapshot, activity: [completed], quality: [quality] };
    expect(toRoutingSnapshot(value)).toEqual(value);
    expect(
      toRoutingSnapshot({
        ...value,
        quality: [{ ...quality, recorded_at_ms: 5 }],
      }),
    ).toBeNull();
  });

  it("rejects overlapping attempts for one logical subtask", () => {
    const replacement = {
      ...snapshot.activity[0],
      child_thread_id: "d2719d93-b823-4a7f-934f-23cbe01c8ab8",
      escalation_count: 1,
    };
    expect(
      toRoutingSnapshot({ ...snapshot, activity: [...snapshot.activity, replacement] }),
    ).toBeNull();
  });

  it("rejects duplicate version-scoped eligibility keys", () => {
    expect(
      toRoutingSnapshot({
        ...snapshot,
        eligibility: [...snapshot.eligibility, { ...snapshot.eligibility[0] }],
      }),
    ).toBeNull();
  });

  it.each([
    (() => {
      const missingSchemaVersion: Record<string, unknown> = { ...snapshot };
      delete missingSchemaVersion.schema_version;
      return missingSchemaVersion;
    })(),
    { ...snapshot, routes: [{ ...snapshot.routes[0], phase: "mystery" }] },
    { ...snapshot, eligibility: [{ ...snapshot.eligibility[0], status: "mystery" }] },
    {
      ...snapshot,
      eligibility: [{ ...snapshot.eligibility[0], requested_model: "gpt-5.6-luna" }],
    },
    { ...snapshot, eligibility: [{ ...snapshot.eligibility[0], depth: 2 }] },
    {
      ...snapshot,
      eligibility: [
        { ...snapshot.eligibility[0], status: "eligible", reason: "host-version-changed" },
      ],
    },
    { ...snapshot, routes: [{ ...snapshot.routes[0], route_key: "not-a-uuid" }] },
    {
      ...snapshot,
      routes: [{ ...snapshot.routes[0], route_key: "00000000-0000-0000-0000-000000000000" }],
    },
    { ...snapshot, routes: [{ ...snapshot.routes[0], created_at_ms: -1 }] },
    {
      ...snapshot,
      routes: [{ ...snapshot.routes[0], created_at_ms: Number.MAX_SAFE_INTEGER + 1 }],
    },
    { ...snapshot, profile_version: "version with task content" },
    { ...snapshot, profile_version: "cm91dGluZy12MQ" },
    { ...snapshot, profile_version: "routing_v1" },
    { ...snapshot, prompt: "CANARY_PRIVATE_PROMPT" },
    { ...snapshot, routes: [{ ...snapshot.routes[0], secret: "CANARY" }] },
    { ...snapshot, activity: [{ ...snapshot.activity[0], reason_codes: ["unknown-code"] }] },
    {
      ...snapshot,
      activity: [
        { ...snapshot.activity[0], parent_thread_id: "d2719d93-b823-4a7f-934f-23cbe01c8ab9" },
      ],
    },
    {
      ...snapshot,
      activity: [
        ...snapshot.activity,
        {
          ...snapshot.activity[0],
          child_thread_id: "d2719d93-b823-4a7f-934f-23cbe01c8ab3",
          is_reviewer: true,
          phase: "completed",
        },
        {
          ...snapshot.activity[0],
          child_thread_id: "d2719d93-b823-4a7f-934f-23cbe01c8ab4",
          subtask_id: "d2719d93-b823-4a7f-934f-23cbe01c8ab5",
          route_kind: "nested",
          parent_thread_id: "d2719d93-b823-4a7f-934f-23cbe01c8ab3",
        },
      ],
    },
    {
      ...snapshot,
      activity: [
        ...snapshot.activity,
        {
          ...snapshot.activity[0],
          child_thread_id: "d2719d93-b823-4a7f-934f-23cbe01c8ab9",
          route_kind: "nested",
          parent_thread_id: "d2719d93-b823-4a7f-934f-23cbe01c8ab1",
          escalation_count: 2,
        },
      ],
    },
  ])("fails closed for malformed or content-bearing payloads", (value) => {
    expect(toRoutingSnapshot(value)).toBeNull();
  });
});
