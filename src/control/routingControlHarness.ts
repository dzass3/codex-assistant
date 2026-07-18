export interface RootBinding {
  conversationId: string;
  routeKey: string;
  observed: boolean;
  parentThreadId: string | null;
}

export type CompatibilityReason =
  | "ready"
  | "unsupported-route"
  | "malformed-route"
  | "incompatible-shell"
  | "ambiguous-composer"
  | "unobserved-root"
  | "child-route"
  | "route-mismatch";

export interface CompatibilityResult {
  compatible: boolean;
  reason: CompatibilityReason;
}

export interface EditorTransaction {
  insertExact(editor: Element, marker: string): { verified: boolean };
}

export type SubmitShortcut = "enter" | "ctrl-enter";

export type RoutingActivity =
  | { state: "classifying" }
  | { state: "implementing"; model: string }
  | { state: "escalation"; model: string }
  | { state: "reviewing"; model: string }
  | { state: "completed"; model?: string }
  | { state: "degraded" }
  | { state: "unavailable" };

type BridgeMessage =
  | {
      v: 1;
      sessionId: string;
      targetId: string;
      type: "compatibility";
      routeId: string;
      compatible: boolean;
      reason: CompatibilityReason;
    }
  | {
      v: 1;
      sessionId: string;
      targetId: string;
      type: "toggle";
      routeId: string;
      enabled: boolean;
    }
  | {
      v: 1;
      sessionId: string;
      targetId: string;
      type: "submit_intent";
      routeId: string;
      routeKey: string;
      submissionId: string;
    }
  | {
      v: 1;
      sessionId: string;
      targetId: string;
      type: "insertion_result";
      routeId: string;
      routeKey: string;
      submissionId: string;
      result: "inserted" | "failed";
    };

export interface MountRoutingControlOptions {
  document: Document;
  pathname(): string;
  binding: RootBinding;
  sessionId: string;
  targetId: string;
  send(message: BridgeMessage): void;
  transaction: EditorTransaction;
  submitShortcut: SubmitShortcut;
}

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
const ROOT_SELECTOR = "[data-codex-composer-root]";
const COMPOSER_SELECTOR = '[data-codex-composer="true"]';
const EDITOR_SELECTOR = ".ProseMirror";

export function routingMarker(routeKey: string): string {
  return `[Codex Assistant Routing v1; route=${routeKey}; policy=1]\nUse $codex-assistant-smart-routing for eligible bounded delegation in this turn. Keep all children native to this root, enforce eligibility/budgets/quality review, and report actual effective models.`;
}

export function probeCompatibility(
  document: Document,
  pathname: string,
  binding: RootBinding,
): CompatibilityResult {
  if (!pathname.startsWith("/local/")) {
    return { compatible: false, reason: "unsupported-route" };
  }
  const route = pathname.slice("/local/".length);
  if (route.includes("/") || !UUID.test(route)) {
    return { compatible: false, reason: "malformed-route" };
  }
  if (route !== binding.conversationId || !UUID.test(binding.routeKey)) {
    return { compatible: false, reason: "route-mismatch" };
  }
  if (!binding.observed) {
    return { compatible: false, reason: "unobserved-root" };
  }
  if (binding.parentThreadId !== null) {
    return { compatible: false, reason: "child-route" };
  }
  if (
    document.querySelector("main.main-surface") === null ||
    document.querySelector("aside.app-shell-left-panel") === null
  ) {
    return { compatible: false, reason: "incompatible-shell" };
  }
  const roots = document.querySelectorAll(ROOT_SELECTOR);
  const composers = document.querySelectorAll(COMPOSER_SELECTOR);
  if (roots.length !== 1 || composers.length !== 1) {
    return { compatible: false, reason: "ambiguous-composer" };
  }
  const root = roots[0];
  const composer = composers[0];
  if (
    root === undefined ||
    composer === undefined ||
    !root.contains(composer) ||
    composer.querySelectorAll(EDITOR_SELECTOR).length !== 1
  ) {
    return { compatible: false, reason: "incompatible-shell" };
  }
  return { compatible: true, reason: "ready" };
}

export function mountRoutingControl(options: MountRoutingControlOptions): RoutingControl {
  return new RoutingControl(options);
}

class RoutingControl {
  readonly #options: MountRoutingControlOptions;
  #controlRoot: HTMLElement | null = null;
  #button: HTMLButtonElement | null = null;
  #composer: Element | null = null;
  #editor: Element | null = null;
  #enabled = false;
  #routingReady = false;
  #degraded = false;
  #markedCurrentTurn = false;
  #submissionCounter = 0;
  #activity: RoutingActivity | null = null;
  #listenersAttached = false;
  #preflightInserted = false;

  constructor(options: MountRoutingControlOptions) {
    this.#options = options;
  }

  mount(): boolean {
    const compatibility = probeCompatibility(
      this.#options.document,
      this.#options.pathname(),
      this.#options.binding,
    );
    this.#sendCompatibility(compatibility);
    if (!compatibility.compatible) {
      this.#unmount();
      return false;
    }
    if (this.#controlRoot?.isConnected) return true;
    const composer = this.#options.document.querySelector(COMPOSER_SELECTOR);
    const editor = composer?.querySelector(EDITOR_SELECTOR) ?? null;
    if (composer === null || editor === null) return false;

    const existing = this.#options.document.querySelector<HTMLElement>(
      "[data-codex-assistant-root]",
    );
    const root = existing ?? this.#options.document.createElement("div");
    root.dataset.codexAssistantRoot = "true";
    let button = root.querySelector<HTMLButtonElement>("[data-codex-assistant-control]");
    if (button === null) {
      button = this.#options.document.createElement("button");
      button.type = "button";
      button.dataset.codexAssistantControl = "true";
      button.setAttribute("aria-pressed", "false");
      root.append(button);
    }
    if (existing === null) composer.append(root);
    this.#controlRoot = root;
    this.#button = button;
    this.#composer = composer;
    this.#editor = editor;
    this.#attachListeners();
    this.#render();
    return true;
  }

  refresh(): boolean {
    const compatibility = probeCompatibility(
      this.#options.document,
      this.#options.pathname(),
      this.#options.binding,
    );
    this.#sendCompatibility(compatibility);
    if (!compatibility.compatible) {
      this.#unmount();
      return false;
    }
    return this.mount();
  }

  setEnabled(enabled: boolean): void {
    if (this.#button === null || this.#degraded || !this.#routingReady) return;
    const message: BridgeMessage = {
      v: 1,
      sessionId: this.#options.sessionId,
      targetId: this.#options.targetId,
      type: "toggle",
      routeId: this.#options.binding.conversationId,
      enabled,
    };
    if (!this.#safeSend(message)) {
      this.#degraded = true;
      this.#enabled = false;
    }
    this.#render();
  }

  syncEnabled(enabled: boolean): void {
    if (!this.#routingReady || this.#degraded) return;
    this.#enabled = enabled;
    if (!enabled) this.#markedCurrentTurn = false;
    this.#render();
  }

  setRoutingReady(ready: boolean): void {
    if (this.#degraded) return;
    this.#routingReady = ready;
    if (!ready) {
      this.#enabled = false;
      this.#markedCurrentTurn = false;
    }
    this.#render();
  }

  setActivity(activity: RoutingActivity): void {
    this.#activity = activity;
    if (activity.state === "degraded" || activity.state === "unavailable") {
      this.#degraded = true;
      this.#enabled = false;
    }
    this.#render();
  }

  insertPreflightDirective(directive: string): boolean {
    if (
      this.#preflightInserted ||
      this.#degraded ||
      this.#editor === null ||
      !this.#editor.matches(":empty") ||
      directive.length < 80 ||
      directive.length > 1024 ||
      !/^Codex Assistant preflight [0-9a-f-]{36}: create exactly one visible native child /.test(
        directive,
      ) ||
      directive.includes("\n") ||
      directive.includes("\0")
    ) {
      return false;
    }
    const inserted =
      this.#options.transaction.insertExact(this.#editor, directive).verified === true;
    if (inserted) this.#preflightInserted = true;
    return inserted;
  }

  #attachListeners(): void {
    if (this.#listenersAttached) return;
    this.#button?.addEventListener("click", this.#onControlClick);
    this.#composer?.addEventListener("keydown", this.#onKeyDown, true);
    this.#options.document.addEventListener("click", this.#onDocumentClick, true);
    this.#editor?.addEventListener("input", this.#onEditorInput);
    this.#listenersAttached = true;
  }

  #detachListeners(): void {
    if (!this.#listenersAttached) return;
    this.#button?.removeEventListener("click", this.#onControlClick);
    this.#composer?.removeEventListener("keydown", this.#onKeyDown, true);
    this.#options.document.removeEventListener("click", this.#onDocumentClick, true);
    this.#editor?.removeEventListener("input", this.#onEditorInput);
    this.#listenersAttached = false;
  }

  #unmount(): void {
    this.#detachListeners();
    this.#controlRoot?.remove();
    this.#controlRoot = null;
    this.#button = null;
    this.#composer = null;
    this.#editor = null;
    this.#enabled = false;
    this.#markedCurrentTurn = false;
  }

  readonly #onControlClick = (): void => {
    this.setEnabled(!this.#enabled);
  };

  readonly #onEditorInput = (): void => {
    this.#markedCurrentTurn = false;
  };

  readonly #onKeyDown = (event: Event): void => {
    if (!(event instanceof KeyboardEvent)) return;
    if (event.key !== "Enter" || event.shiftKey || event.altKey || event.metaKey) return;
    if (event.isComposing || event.keyCode === 229) return;
    const isSubmission =
      this.#options.submitShortcut === "ctrl-enter" ? event.ctrlKey : !event.ctrlKey;
    if (!isSubmission) return;
    this.#insertForSubmission();
  };

  readonly #onDocumentClick = (event: Event): void => {
    const target = event.target;
    if (target === null || !("closest" in target)) return;
    const button = (target as Element).closest<HTMLButtonElement>("button");
    if (button === null || !this.#composer?.contains(button)) return;
    if (
      !button.matches("[data-codex-submit]") ||
      button.matches("[data-codex-stop]") ||
      button.disabled ||
      button.getAttribute("aria-disabled") === "true"
    ) {
      return;
    }
    this.#insertForSubmission();
  };

  #insertForSubmission(): void {
    if (
      !this.#enabled ||
      this.#degraded ||
      this.#markedCurrentTurn ||
      this.#editor === null ||
      this.#hasActiveOverlay()
    ) {
      return;
    }
    this.#submissionCounter += 1;
    const submissionId = `${this.#options.sessionId}:${this.#submissionCounter}`;
    const intent: BridgeMessage = {
      v: 1,
      sessionId: this.#options.sessionId,
      targetId: this.#options.targetId,
      type: "submit_intent",
      routeId: this.#options.binding.conversationId,
      routeKey: this.#options.binding.routeKey,
      submissionId,
    };
    if (!this.#safeSend(intent)) {
      this.#degraded = true;
      this.#enabled = false;
      this.#render();
      return;
    }
    const result = this.#options.transaction.insertExact(
      this.#editor,
      routingMarker(this.#options.binding.routeKey),
    );
    const inserted = result.verified === true;
    this.#safeSend({
      v: 1,
      sessionId: this.#options.sessionId,
      targetId: this.#options.targetId,
      type: "insertion_result",
      routeId: this.#options.binding.conversationId,
      routeKey: this.#options.binding.routeKey,
      submissionId,
      result: inserted ? "inserted" : "failed",
    });
    if (inserted) {
      this.#markedCurrentTurn = true;
    } else {
      this.#degraded = true;
      this.#enabled = false;
    }
    this.#render();
  }

  #hasActiveOverlay(): boolean {
    return Array.from(
      this.#options.document.querySelectorAll<HTMLElement>('[role="dialog"], [role="menu"]'),
    ).some((overlay) => !overlay.hidden && overlay.getAttribute("aria-hidden") !== "true");
  }

  #render(): void {
    if (this.#button === null) return;
    this.#button.setAttribute("aria-pressed", String(this.#enabled));
    this.#button.disabled = !this.#routingReady || this.#degraded;
    if (this.#degraded) {
      this.#button.textContent = "Codex Assistant · Degraded";
      return;
    }
    if (!this.#routingReady) {
      this.#button.textContent = "Codex Assistant · Preflight required";
      return;
    }
    if (!this.#enabled) {
      this.#button.textContent = "Codex Assistant · Smart Routing · Off";
      return;
    }
    const prefix = `Codex Assistant · Enabled · ${this.#options.binding.routeKey}`;
    if (this.#activity === null) {
      this.#button.textContent = prefix;
      return;
    }
    const label = activityLabel(this.#activity);
    this.#button.textContent = `${prefix} · ${label}`;
  }

  #sendCompatibility(result: CompatibilityResult): void {
    this.#safeSend({
      v: 1,
      sessionId: this.#options.sessionId,
      targetId: this.#options.targetId,
      type: "compatibility",
      routeId: this.#options.binding.conversationId,
      compatible: result.compatible,
      reason: result.reason,
    });
  }

  #safeSend(message: BridgeMessage): boolean {
    try {
      this.#options.send(message);
      return true;
    } catch {
      return false;
    }
  }
}

function activityLabel(activity: RoutingActivity): string {
  switch (activity.state) {
    case "classifying":
      return "Classifying";
    case "implementing":
      return `${activity.model} · Implementing`;
    case "escalation":
      return `${activity.model} · Escalation`;
    case "reviewing":
      return `${activity.model} · Reviewing`;
    case "completed":
      return activity.model === undefined ? "Completed" : `${activity.model} · Completed`;
    case "degraded":
      return "Degraded";
    case "unavailable":
      return "Unavailable";
  }
}
