(() => {
  "use strict";

  const GLOBAL_NAME = "__codexAssistantControlV1";
  const BOOTSTRAP_NAME = "__codexAssistantBootstrapV1";
  const ROOT_ATTRIBUTE = "data-codex-assistant-root";
  const CONTROL_ATTRIBUTE = "data-codex-assistant-control";
  const ROOT_SELECTOR = "[data-codex-composer-root]";
  const COMPOSER_SELECTOR = '[data-codex-composer="true"]';
  const EDITOR_SELECTOR = ".ProseMirror";
  const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
  const MODELS = new Set(["gpt-5.3-codex-spark", "gpt-5.6-luna", "gpt-5.6-terra", "gpt-5.6-sol"]);

  const bootstrap = globalThis[BOOTSTRAP_NAME];
  if (!validBootstrap(bootstrap) || typeof globalThis.codexAssistant !== "function") return;
  const existing = globalThis[GLOBAL_NAME];
  if (existing && typeof existing.refresh === "function") {
    if (
      existing.routeId === bootstrap.routeId &&
      existing.routeKey === bootstrap.routeKey &&
      existing.sessionId === bootstrap.sessionId &&
      existing.targetId === bootstrap.targetId
    ) {
      existing.refresh();
      return;
    }
    if (typeof existing.destroy !== "function") return;
    existing.destroy();
  }

  let controlRoot = null;
  let button = null;
  let composer = null;
  let editor = null;
  let enabled = false;
  let routingReady = false;
  let degraded = false;
  let markedCurrentTurn = false;
  let submissionCounter = 0;
  let activity = null;
  let listenersAttached = false;
  let preflightInserted = false;

  const api = Object.freeze({
    routeId: bootstrap.routeId,
    routeKey: bootstrap.routeKey,
    sessionId: bootstrap.sessionId,
    targetId: bootstrap.targetId,
    refresh,
    destroy,
    updateActivity,
    insertPreflightDirective,
    setRoutingReady,
    syncEnabled,
  });
  globalThis[GLOBAL_NAME] = api;

  mount();

  function validBootstrap(value) {
    if (!value || typeof value !== "object" || Array.isArray(value)) return false;
    const expected = [
      "v",
      "sessionId",
      "targetId",
      "routeId",
      "routeKey",
      "observed",
      "parentThreadId",
      "submitShortcut",
      "css",
    ];
    if (!exactKeys(value, expected)) return false;
    return (
      value.v === 1 &&
      safeId(value.sessionId) &&
      safeId(value.targetId) &&
      UUID.test(value.routeId) &&
      UUID.test(value.routeKey) &&
      value.observed === true &&
      value.parentThreadId === null &&
      (value.submitShortcut === "enter" || value.submitShortcut === "ctrl-enter") &&
      typeof value.css === "string" &&
      value.css.length <= 16384
    );
  }

  function exactKeys(value, expected) {
    const keys = Object.keys(value);
    return keys.length === expected.length && expected.every((key) => keys.includes(key));
  }

  function safeId(value) {
    return (
      typeof value === "string" &&
      value.length >= 1 &&
      value.length <= 128 &&
      /^[A-Za-z0-9._:-]+$/.test(value)
    );
  }

  function compatibility() {
    const path = location.pathname;
    if (!path.startsWith("/local/")) return result(false, "unsupported-route");
    const route = path.slice("/local/".length);
    if (route.includes("/") || !UUID.test(route)) return result(false, "malformed-route");
    if (route !== bootstrap.routeId) return result(false, "route-mismatch");
    if (!bootstrap.observed) return result(false, "unobserved-root");
    if (bootstrap.parentThreadId !== null) return result(false, "child-route");
    if (
      !document.querySelector("main.main-surface") ||
      !document.querySelector("aside.app-shell-left-panel")
    ) {
      return result(false, "incompatible-shell");
    }
    const roots = document.querySelectorAll(ROOT_SELECTOR);
    const composers = document.querySelectorAll(COMPOSER_SELECTOR);
    if (roots.length !== 1 || composers.length !== 1) {
      return result(false, "ambiguous-composer");
    }
    const root = roots[0];
    const candidate = composers[0];
    if (!root.contains(candidate) || candidate.querySelectorAll(EDITOR_SELECTOR).length !== 1) {
      return result(false, "incompatible-shell");
    }
    return result(true, "ready");
  }

  function result(compatible, reason) {
    return { compatible, reason };
  }

  function mount() {
    const status = compatibility();
    emit({
      type: "compatibility",
      routeId: bootstrap.routeId,
      compatible: status.compatible,
      reason: status.reason,
    });
    if (!status.compatible) {
      unmount();
      return false;
    }
    if (controlRoot && controlRoot.isConnected) return true;
    composer = document.querySelector(COMPOSER_SELECTOR);
    editor = composer.querySelector(EDITOR_SELECTOR);
    controlRoot = document.querySelector(`[${ROOT_ATTRIBUTE}]`) || document.createElement("div");
    controlRoot.setAttribute(ROOT_ATTRIBUTE, "true");
    button = controlRoot.querySelector(`[${CONTROL_ATTRIBUTE}]`);
    if (!button) {
      button = document.createElement("button");
      button.type = "button";
      button.setAttribute(CONTROL_ATTRIBUTE, "true");
      button.setAttribute("aria-pressed", "false");
      controlRoot.append(button);
    }
    if (!controlRoot.isConnected) composer.append(controlRoot);
    installStyle();
    attachListeners();
    render();
    return true;
  }

  function installStyle() {
    if (!bootstrap.css || controlRoot.querySelector("[data-codex-assistant-style]")) return;
    const style = document.createElement("style");
    style.setAttribute("data-codex-assistant-style", "true");
    style.replaceChildren(document.createTextNode(bootstrap.css));
    controlRoot.prepend(style);
  }

  function refresh() {
    return mount();
  }

  function destroy() {
    unmount();
    if (globalThis[GLOBAL_NAME] === api) delete globalThis[GLOBAL_NAME];
  }

  function unmount() {
    detachListeners();
    if (controlRoot) controlRoot.remove();
    controlRoot = null;
    button = null;
    composer = null;
    editor = null;
    enabled = false;
    markedCurrentTurn = false;
  }

  function attachListeners() {
    if (listenersAttached) return;
    button.addEventListener("click", onControlClick);
    composer.addEventListener("keydown", onKeyDown, true);
    document.addEventListener("click", onDocumentClick, true);
    editor.addEventListener("input", onEditorInput);
    addEventListener("popstate", refresh);
    listenersAttached = true;
  }

  function detachListeners() {
    if (!listenersAttached) return;
    if (button) button.removeEventListener("click", onControlClick);
    if (composer) composer.removeEventListener("keydown", onKeyDown, true);
    document.removeEventListener("click", onDocumentClick, true);
    if (editor) editor.removeEventListener("input", onEditorInput);
    removeEventListener("popstate", refresh);
    listenersAttached = false;
  }

  function onControlClick() {
    if (degraded || !routingReady) return;
    const requested = !enabled;
    if (
      !emit({
        type: "toggle",
        routeId: bootstrap.routeId,
        enabled: requested,
      })
    ) {
      enterDegraded();
      render();
      return;
    }
  }

  function onEditorInput() {
    markedCurrentTurn = false;
  }

  function onKeyDown(event) {
    if (
      event.key !== "Enter" ||
      event.shiftKey ||
      event.altKey ||
      event.metaKey ||
      event.isComposing ||
      event.keyCode === 229
    ) {
      return;
    }
    const submit = bootstrap.submitShortcut === "ctrl-enter" ? event.ctrlKey : !event.ctrlKey;
    if (submit) insertForSubmission();
  }

  function onDocumentClick(event) {
    if (!event.target || typeof event.target.closest !== "function") return;
    const candidate = event.target.closest("button");
    if (!candidate || !composer || !composer.contains(candidate)) return;
    if (
      !candidate.matches("[data-codex-submit]") ||
      candidate.matches("[data-codex-stop]") ||
      candidate.disabled ||
      candidate.getAttribute("aria-disabled") === "true"
    ) {
      return;
    }
    insertForSubmission();
  }

  function insertForSubmission() {
    if (!enabled || degraded || markedCurrentTurn || !editor || activeOverlay()) return;
    submissionCounter += 1;
    const submissionId = `${bootstrap.sessionId}:${submissionCounter}`;
    if (
      !emit({
        type: "submit_intent",
        routeId: bootstrap.routeId,
        routeKey: bootstrap.routeKey,
        submissionId,
      })
    ) {
      enterDegraded();
      return;
    }
    const inserted = insertExact(marker());
    emit({
      type: "insertion_result",
      routeId: bootstrap.routeId,
      routeKey: bootstrap.routeKey,
      submissionId,
      result: inserted ? "inserted" : "failed",
    });
    if (inserted) markedCurrentTurn = true;
    else enterDegraded();
    render();
  }

  function insertExact(value) {
    const selection = document.getSelection();
    if (!selection || selection.rangeCount !== 1 || !selection.isCollapsed) return false;
    const beforeNode = selection.focusNode;
    const beforeOffset = selection.focusOffset;
    if (!beforeNode || !editor.contains(beforeNode)) return false;
    if (!document.execCommand("insertText", false, value)) return false;
    const afterNode = selection.focusNode;
    const afterOffset = selection.focusOffset;
    if (!afterNode || !editor.contains(afterNode) || afterNode.nodeType !== Node.TEXT_NODE) {
      return false;
    }
    const start = afterNode === beforeNode ? beforeOffset : afterOffset - value.length;
    if (start < 0 || afterOffset - start !== value.length) return false;
    return afterNode.data.slice(start, afterOffset) === value;
  }

  function marker() {
    return `[Codex Assistant Routing v1; route=${bootstrap.routeKey}; policy=1]\nUse $codex-assistant-smart-routing for eligible bounded delegation in this turn. Keep all children native to this root, enforce eligibility/budgets/quality review, and report actual effective models.`;
  }

  function activeOverlay() {
    return Array.from(document.querySelectorAll('[role="dialog"], [role="menu"]')).some(
      (overlay) => !overlay.hidden && overlay.getAttribute("aria-hidden") !== "true",
    );
  }

  function updateActivity(next) {
    if (!validActivity(next)) {
      enterDegraded();
      return false;
    }
    activity = next;
    if (next.state === "degraded" || next.state === "unavailable") enterDegraded();
    render();
    return true;
  }

  function insertPreflightDirective(value) {
    if (
      preflightInserted ||
      degraded ||
      !editor ||
      !editor.matches(":empty") ||
      typeof value !== "string" ||
      value.length < 80 ||
      value.length > 1024 ||
      !/^Codex Assistant preflight [0-9a-f-]{36}: create exactly one visible native child /.test(
        value,
      ) ||
      value.includes("\n") ||
      value.includes("\0")
    ) {
      return false;
    }
    editor.focus();
    const selection = document.getSelection();
    if (!selection) return false;
    const range = document.createRange();
    range.selectNodeContents(editor);
    range.collapse(false);
    selection.removeAllRanges();
    selection.addRange(range);
    const inserted = insertExact(value);
    if (inserted) preflightInserted = true;
    return inserted;
  }

  function validActivity(value) {
    if (!value || typeof value !== "object" || Array.isArray(value)) return false;
    if (
      value.state === "classifying" ||
      value.state === "degraded" ||
      value.state === "unavailable"
    ) {
      return exactKeys(value, ["state"]);
    }
    if (value.state === "completed" && exactKeys(value, ["state"])) return true;
    return (
      ["implementing", "escalation", "reviewing", "completed"].includes(value.state) &&
      exactKeys(value, ["state", "model"]) &&
      MODELS.has(value.model)
    );
  }

  function enterDegraded() {
    degraded = true;
    enabled = false;
  }

  function setRoutingReady(ready) {
    if (degraded || typeof ready !== "boolean") return false;
    routingReady = ready;
    if (!ready) {
      enabled = false;
      markedCurrentTurn = false;
    }
    render();
    return true;
  }

  function syncEnabled(value) {
    if (!routingReady || degraded || typeof value !== "boolean") return false;
    enabled = value;
    if (!enabled) markedCurrentTurn = false;
    render();
    return true;
  }

  function render() {
    if (!button) return;
    button.setAttribute("aria-pressed", String(enabled));
    button.disabled = !routingReady || degraded;
    button.setAttribute("data-state", degraded ? "degraded" : enabled ? "enabled" : "off");
    let label = "Codex Assistant · Smart Routing · Off";
    if (degraded) label = "Codex Assistant · Degraded";
    else if (!routingReady) label = "Codex Assistant · Preflight required";
    else if (enabled) {
      label = `Codex Assistant · Enabled · ${bootstrap.routeKey}`;
      if (activity) label += ` · ${activityLabel(activity)}`;
    }
    button.replaceChildren(document.createTextNode(label));
  }

  function activityLabel(value) {
    const labels = {
      classifying: "Classifying",
      implementing: "Implementing",
      escalation: "Escalation",
      reviewing: "Reviewing",
      completed: "Completed",
      degraded: "Degraded",
      unavailable: "Unavailable",
    };
    return value.model ? `${value.model} · ${labels[value.state]}` : labels[value.state];
  }

  function emit(payload) {
    const message = {
      v: 1,
      sessionId: bootstrap.sessionId,
      targetId: bootstrap.targetId,
      ...payload,
    };
    try {
      globalThis.codexAssistant(JSON.stringify(message));
      return true;
    } catch {
      return false;
    }
  }
})();
