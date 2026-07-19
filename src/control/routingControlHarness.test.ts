import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { describe, expect, it, vi } from "vitest";

import {
  mountRoutingControl,
  probeCompatibility,
  routingMarker,
  type EditorTransaction,
  type RootBinding,
} from "./routingControlHarness";

const ROOT_ID = "7d47a800-c734-4f9a-a56c-55d875ea1cab";
const ROUTE_KEY = "6e90c53a-b93e-44d7-aeb8-9880ee199388";
const OTHER_ROOT_ID = "bd542ff9-fc21-41c5-957d-c45adc842d21";
const OTHER_ROUTE_KEY = "fb49751f-64ae-4613-89de-5997d766486a";

function fixture(name: string): Document {
  const html = readFileSync(
    resolve(process.cwd(), "src-tauri", "resources", "control", "fixtures", `${name}.html`),
    "utf8",
  );
  return new DOMParser().parseFromString(html, "text/html");
}

function binding(overrides: Partial<RootBinding> = {}): RootBinding {
  return {
    conversationId: ROOT_ID,
    routeKey: ROUTE_KEY,
    observed: true,
    parentThreadId: null,
    ...overrides,
  };
}

function transaction(): EditorTransaction & { inserted: string[] } {
  const inserted: string[] = [];
  return {
    inserted,
    insertExact(_editor, marker) {
      inserted.push(marker);
      return { verified: true };
    },
  };
}

describe("routing control compatibility", () => {
  it("accepts only one complete local root composer confirmed by Rust", () => {
    expect(probeCompatibility(fixture("local-root"), `/local/${ROOT_ID}`, binding())).toEqual({
      compatible: true,
      reason: "ready",
    });

    const cases: Array<[Document, string, RootBinding, string]> = [
      [
        fixture("local-child"),
        `/local/${ROOT_ID}`,
        binding({ parentThreadId: ROUTE_KEY }),
        "child-route",
      ],
      [fixture("local-root"), "/remote", binding(), "unsupported-route"],
      [fixture("local-root"), "/work/conversation", binding(), "unsupported-route"],
      [fixture("local-root"), "/hotkey-window/thread", binding(), "unsupported-route"],
      [fixture("local-root"), "/local/not-a-uuid", binding(), "malformed-route"],
      [fixture("incompatible"), `/local/${ROOT_ID}`, binding(), "incompatible-shell"],
      [fixture("local-root"), `/local/${ROOT_ID}`, binding({ observed: false }), "unobserved-root"],
      [
        fixture("local-root"),
        `/local/${ROOT_ID}`,
        binding({ conversationId: ROUTE_KEY }),
        "route-mismatch",
      ],
    ];
    const duplicate = fixture("local-root");
    duplicate
      .querySelector("main")
      ?.append(duplicate.querySelector("[data-codex-composer-root]")!.cloneNode(true));
    cases.push([duplicate, `/local/${ROOT_ID}`, binding(), "ambiguous-composer"]);

    for (const [document, path, root, reason] of cases) {
      expect(probeCompatibility(document, path, root)).toEqual({ compatible: false, reason });
    }
  });
});

describe("mounted routing control", () => {
  it("keeps routing locked until the verified preflight explicitly activates it", () => {
    const document = fixture("local-root");
    const send = vi.fn();
    const control = mountRoutingControl({
      document,
      pathname: () => `/local/${ROOT_ID}`,
      binding: binding(),
      sessionId: "session-1",
      targetId: "target-1",
      send,
      transaction: transaction(),
      submitShortcut: "enter",
    });
    control.mount();
    const button = document.querySelector<HTMLButtonElement>("[data-codex-assistant-control]")!;

    expect(button.disabled).toBe(true);
    button.click();
    expect(send).not.toHaveBeenCalledWith(expect.objectContaining({ type: "toggle" }));

    control.setRoutingReady(true);
    expect(button.disabled).toBe(false);
    button.click();
    expect(send).toHaveBeenCalledWith(expect.objectContaining({ type: "toggle", enabled: true }));
    expect(button.textContent).toContain("Off");
    control.syncEnabled(true);
    expect(button.textContent).toContain("Enabled");
  });

  it("boots the packaged runtime asset idempotently through the owned CDP bootstrap", () => {
    const document = fixture("local-root");
    const runtime = readFileSync(
      resolve(process.cwd(), "src-tauri", "resources", "control", "routing-control.js"),
      "utf8",
    );
    const bridge = vi.fn();
    const page = window as typeof window & Record<string, unknown>;
    delete page["__codexAssistantControlV1"];
    page["__codexAssistantBootstrapV1"] = {
      v: 1,
      sessionId: "session-1",
      targetId: "target-1",
      routeId: ROOT_ID,
      routeKey: ROUTE_KEY,
      observed: true,
      parentThreadId: null,
      submitShortcut: "enter",
      css: "",
    };
    page.codexAssistant = bridge;
    const execute = new Function(
      "globalThis",
      "document",
      "location",
      "addEventListener",
      "removeEventListener",
      "Node",
      runtime,
    );
    const location = { pathname: `/local/${ROOT_ID}` };
    const run = () =>
      execute(
        page,
        document,
        location,
        page.addEventListener.bind(page),
        page.removeEventListener.bind(page),
        page.Node,
      );
    run();
    run();
    expect(document.querySelectorAll("[data-codex-assistant-control]")).toHaveLength(1);
    document.querySelector<HTMLButtonElement>("[data-codex-assistant-control]")!.click();
    expect(bridge).not.toHaveBeenCalledWith(expect.stringContaining('"type":"toggle"'));
    const runtimeControl = page["__codexAssistantControlV1"] as {
      destroy(): void;
      setRoutingReady(ready: boolean): boolean;
      syncEnabled(enabled: boolean): boolean;
    };
    runtimeControl.setRoutingReady(true);
    document.querySelector<HTMLButtonElement>("[data-codex-assistant-control]")!.click();
    expect(bridge).toHaveBeenCalledWith(expect.stringContaining('"type":"toggle"'));
    runtimeControl.syncEnabled(true);
    expect(document.querySelector("[data-codex-assistant-control]")?.textContent).toContain(
      "Enabled",
    );
    runtimeControl.destroy();
    delete page["__codexAssistantBootstrapV1"];
    delete page.codexAssistant;
  });

  it("rebinds the packaged native control when the same target navigates to another root", () => {
    const document = fixture("local-root");
    const runtime = readFileSync(
      resolve(process.cwd(), "src-tauri", "resources", "control", "routing-control.js"),
      "utf8",
    );
    const page = window as typeof window & Record<string, unknown>;
    delete page["__codexAssistantControlV1"];
    page.codexAssistant = vi.fn();
    const location = { pathname: `/local/${ROOT_ID}` };
    const execute = new Function(
      "globalThis",
      "document",
      "location",
      "addEventListener",
      "removeEventListener",
      "Node",
      runtime,
    );
    const run = () =>
      execute(
        page,
        document,
        location,
        page.addEventListener.bind(page),
        page.removeEventListener.bind(page),
        page.Node,
      );

    page["__codexAssistantBootstrapV1"] = {
      v: 1,
      sessionId: "session-root-1",
      targetId: "target-1",
      routeId: ROOT_ID,
      routeKey: ROUTE_KEY,
      observed: true,
      parentThreadId: null,
      submitShortcut: "enter",
      css: "",
    };
    run();
    expect((page["__codexAssistantControlV1"] as { routeId: string }).routeId).toBe(ROOT_ID);

    location.pathname = `/local/${OTHER_ROOT_ID}`;
    page["__codexAssistantBootstrapV1"] = {
      v: 1,
      sessionId: "session-root-2",
      targetId: "target-1",
      routeId: OTHER_ROOT_ID,
      routeKey: OTHER_ROUTE_KEY,
      observed: true,
      parentThreadId: null,
      submitShortcut: "enter",
      css: "",
    };
    run();

    expect((page["__codexAssistantControlV1"] as { routeId: string }).routeId).toBe(OTHER_ROOT_ID);
    expect(document.querySelectorAll("[data-codex-assistant-control]")).toHaveLength(1);
    (page["__codexAssistantControlV1"] as { destroy(): void }).destroy();
    delete page["__codexAssistantBootstrapV1"];
    delete page.codexAssistant;
  });

  it("injects one namespaced chip idempotently and emits exact bridge messages", () => {
    const document = fixture("local-root");
    const send = vi.fn();
    const control = mountRoutingControl({
      document,
      pathname: () => `/local/${ROOT_ID}`,
      binding: binding(),
      sessionId: "session-1",
      targetId: "target-1",
      send,
      transaction: transaction(),
      submitShortcut: "enter",
    });
    expect(control.mount()).toBe(true);
    expect(control.mount()).toBe(true);
    expect(document.querySelectorAll("[data-codex-assistant-control]")).toHaveLength(1);
    control.setRoutingReady(true);
    control.setEnabled(true);
    control.syncEnabled(true);
    expect(document.querySelector("[data-codex-assistant-control]")?.textContent).toContain(
      "Enabled",
    );
    expect(document.querySelector("[data-codex-assistant-control]")?.textContent).toContain(
      ROUTE_KEY,
    );
    expect(send).toHaveBeenCalledWith({
      v: 1,
      sessionId: "session-1",
      targetId: "target-1",
      type: "toggle",
      routeId: ROOT_ID,
      enabled: true,
    });
  });

  it("marks genuine keyboard submissions but ignores IME, Shift+Enter and wrong shortcuts", () => {
    const document = fixture("local-root");
    const editor = document.querySelector(".ProseMirror")!;
    const tx = transaction();
    const control = mountRoutingControl({
      document,
      pathname: () => `/local/${ROOT_ID}`,
      binding: binding(),
      sessionId: "session-1",
      targetId: "target-1",
      send: vi.fn(),
      transaction: tx,
      submitShortcut: "ctrl-enter",
    });
    control.mount();
    control.setRoutingReady(true);
    control.setEnabled(true);
    control.syncEnabled(true);

    editor.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    editor.dispatchEvent(
      new KeyboardEvent("keydown", { key: "Enter", shiftKey: true, bubbles: true }),
    );
    editor.dispatchEvent(
      new KeyboardEvent("keydown", {
        key: "Enter",
        ctrlKey: true,
        isComposing: true,
        bubbles: true,
      }),
    );
    expect(tx.inserted).toHaveLength(0);
    editor.dispatchEvent(
      new KeyboardEvent("keydown", { key: "Enter", ctrlKey: true, bubbles: true }),
    );
    expect(tx.inserted).toEqual([routingMarker(ROUTE_KEY)]);
  });

  it("handles send controls, modal safety, repeat turns, disable and activity states", () => {
    const document = fixture("local-root");
    const tx = transaction();
    const send = vi.fn();
    const control = mountRoutingControl({
      document,
      pathname: () => `/local/${ROOT_ID}`,
      binding: binding(),
      sessionId: "session-1",
      targetId: "target-1",
      send,
      transaction: tx,
      submitShortcut: "enter",
    });
    control.mount();
    control.setRoutingReady(true);
    control.setEnabled(true);
    control.syncEnabled(true);
    const submit = document.querySelector<HTMLButtonElement>("[data-codex-submit]")!;
    const stop = document.querySelector<HTMLButtonElement>("[data-codex-stop]")!;
    stop.click();
    expect(tx.inserted).toHaveLength(0);
    submit.disabled = true;
    submit.click();
    expect(tx.inserted).toHaveLength(0);
    submit.disabled = false;
    const dialog = document.createElement("div");
    dialog.setAttribute("role", "dialog");
    document.body.append(dialog);
    submit.click();
    expect(tx.inserted).toHaveLength(0);
    dialog.remove();
    submit.click();
    submit.click();
    expect(tx.inserted).toEqual([routingMarker(ROUTE_KEY)]);
    document.querySelector(".ProseMirror")!.dispatchEvent(new Event("input", { bubbles: true }));
    submit.click();
    expect(tx.inserted).toHaveLength(2);
    control.setActivity({ state: "implementing", model: "gpt-5.6-luna" });
    expect(document.querySelector("[data-codex-assistant-control]")?.textContent).toContain(
      "gpt-5.6-luna · Implementing",
    );
    control.setEnabled(false);
    control.syncEnabled(false);
    document.querySelector(".ProseMirror")!.dispatchEvent(new Event("input", { bubbles: true }));
    submit.click();
    expect(tx.inserted).toHaveLength(2);
  });

  it("fails closed on insertion failure and route navigation", () => {
    const document = fixture("local-root");
    let path = `/local/${ROOT_ID}`;
    const send = vi.fn();
    const control = mountRoutingControl({
      document,
      pathname: () => path,
      binding: binding(),
      sessionId: "session-1",
      targetId: "target-1",
      send,
      transaction: { insertExact: () => ({ verified: false }) },
      submitShortcut: "enter",
    });
    control.mount();
    control.setRoutingReady(true);
    control.setEnabled(true);
    control.syncEnabled(true);
    document.querySelector<HTMLButtonElement>("[data-codex-submit]")!.click();
    expect(document.querySelector("[data-codex-assistant-control]")?.textContent).toContain(
      "Degraded",
    );
    expect(send).toHaveBeenCalledWith(
      expect.objectContaining({ type: "insertion_result", result: "failed" }),
    );
    path = "/remote";
    expect(control.refresh()).toBe(false);
    expect(document.querySelector("[data-codex-assistant-control]")).toBeNull();
  });

  it("inserts one bounded preflight directive only into the current empty root editor", () => {
    const document = fixture("local-root");
    const tx = transaction();
    const control = mountRoutingControl({
      document,
      pathname: () => `/local/${ROOT_ID}`,
      binding: binding(),
      sessionId: "session-1",
      targetId: "target-1",
      send: vi.fn(),
      transaction: tx,
      submitShortcut: "enter",
    });
    control.mount();
    const directive = `Codex Assistant preflight ${ROUTE_KEY}: create exactly one visible native child from the current root using profile codex_assistant_luna with fork_turns="none". The child performs no user work and reports only native availability.`;

    expect(control.insertPreflightDirective(directive)).toBe(true);
    expect(control.insertPreflightDirective(directive)).toBe(false);
    expect(tx.inserted).toEqual([directive]);

    const occupied = fixture("local-root");
    occupied.querySelector(".ProseMirror")?.append(occupied.createTextNode("existing user draft"));
    const guarded = mountRoutingControl({
      document: occupied,
      pathname: () => `/local/${ROOT_ID}`,
      binding: binding(),
      sessionId: "session-2",
      targetId: "target-2",
      send: vi.fn(),
      transaction: transaction(),
      submitShortcut: "enter",
    });
    guarded.mount();
    expect(guarded.insertPreflightDirective(directive)).toBe(false);
  });
});
