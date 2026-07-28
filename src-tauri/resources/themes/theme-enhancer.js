/* oxlint-disable no-unused-expressions -- Rust embeds this reviewed function source. */
/* oxlint-disable unicorn/consistent-function-scoping -- Helpers stay inside the isolated injector. */
() => {
  "use strict";

  const WELCOME_SELECTOR = "[data-codex-assistant-theme-welcome]";
  const MAIN_SELECTORS = [
    "main.main-surface",
    'main[role="main"]',
    '[data-codex-main="true"]',
    'section[role="main"]',
  ];
  const COMPOSER_SELECTORS = [
    ".composer-surface-chrome",
    'form[aria-label*="message" i]',
    'form[data-codex-composer="true"]',
  ];
  const INPUT_SELECTORS = [
    "textarea[aria-label]",
    "textarea",
    '[contenteditable="true"][role="textbox"]',
    "input[aria-label]",
  ];
  const HOME_EVIDENCE_SELECTORS = [
    '[data-codex-home-state="true"]',
    '[data-page-kind="home"]',
    '[data-state="empty"]',
    '[data-testid*="empty"]',
  ];
  const CONVERSATION_SELECTORS = [
    '[data-codex-conversation="true"]',
    "[data-message-author-role]",
    "[data-message-role]",
    "[data-user-message-bubble]",
    '[data-testid*="message"]',
    "article",
  ];
  const ACTIONS = [
    {
      id: "create-project",
      label: "创建项目",
      detail: "从目标和技术栈开始",
      prompt: "帮我创建一个新项目，并先梳理目标、技术栈和实施步骤。",
    },
    {
      id: "open-project",
      label: "打开项目",
      detail: "继续本地已有工作",
      prompt: "帮我打开并熟悉一个本地项目。",
    },
    {
      id: "analyze-repository",
      label: "分析仓库",
      detail: "理解结构、风险与依赖",
      prompt: "请分析当前仓库的结构、关键模块、依赖和主要风险。",
    },
    {
      id: "execute-task",
      label: "执行任务",
      detail: "把计划交给 Codex",
      prompt: "请执行这个任务：",
    },
  ];

  const present = (element) => {
    if (!element || typeof element.getBoundingClientRect !== "function") return false;
    const style = getComputedStyle(element);
    const rect = element.getBoundingClientRect();
    return (
      style.display !== "none" &&
      style.visibility !== "hidden" &&
      Number(style.opacity) !== 0 &&
      rect.width > 0 &&
      rect.height > 0
    );
  };

  const firstPresent = (root, selectors) => {
    for (const selector of selectors) {
      const candidate = root.querySelector(selector);
      if (present(candidate)) return candidate;
    }
    return null;
  };

  const removeWelcome = () => {
    document.querySelector(WELCOME_SELECTOR)?.remove();
    document.documentElement.removeAttribute("data-codex-assistant-theme-home");
  };

  const findComposerInput = (main) => {
    const composer = firstPresent(main, COMPOSER_SELECTORS);
    return composer ? firstPresent(composer, INPUT_SELECTORS) : null;
  };

  const setNativeValue = (input, value) => {
    if (input instanceof HTMLTextAreaElement || input instanceof HTMLInputElement) {
      const prototype =
        input instanceof HTMLTextAreaElement
          ? HTMLTextAreaElement.prototype
          : HTMLInputElement.prototype;
      const setter = Object.getOwnPropertyDescriptor(prototype, "value")?.set;
      if (setter) setter.call(input, value);
      else input.value = value;
    } else {
      input.replaceChildren(document.createTextNode(value));
    }
    input.dispatchEvent(
      new InputEvent("input", {
        bubbles: true,
        composed: true,
        inputType: "insertText",
        data: value,
      }),
    );
    input.focus({ preventScroll: true });
  };

  const triggerAction = (main, action) => {
    if (action.id === "open-project") {
      const nativeButton = [...main.querySelectorAll("button")].find((button) => {
        if (!present(button) || button.closest(WELCOME_SELECTOR)) return false;
        const label = `${button.getAttribute("aria-label") ?? ""} ${
          button.getAttribute("data-testid") ?? ""
        }`;
        return /打开项目|open project/i.test(label);
      });
      if (nativeButton) {
        nativeButton.click();
        return;
      }
    }
    const input = findComposerInput(main);
    if (input) setNativeValue(input, action.prompt);
  };

  const isEmptyHome = (main) => {
    if (!findComposerInput(main)) return false;
    const hasHomeEvidence = HOME_EVIDENCE_SELECTORS.some((selector) =>
      [...main.querySelectorAll(selector)].some(present),
    );
    const hasConversation = CONVERSATION_SELECTORS.some((selector) =>
      [...main.querySelectorAll(selector)].some(
        (element) => !element.closest(WELCOME_SELECTOR) && present(element),
      ),
    );
    return !hasConversation && (hasHomeEvidence || main.querySelectorAll("article").length === 0);
  };

  const createWelcome = (main) => {
    const welcome = document.createElement("section");
    welcome.setAttribute("data-codex-assistant-theme-welcome", "true");
    welcome.setAttribute("aria-label", "新建任务快捷入口");

    const copy = document.createElement("div");
    copy.setAttribute("data-codex-assistant-welcome-copy", "true");
    const title = document.createElement("h2");
    title.replaceChildren(document.createTextNode("想构建什么？"));
    const subtitle = document.createElement("p");
    subtitle.replaceChildren(document.createTextNode("选择一个起点，或直接在下方描述你的任务。"));
    copy.append(title, subtitle);

    const grid = document.createElement("div");
    grid.setAttribute("data-codex-assistant-welcome-grid", "true");
    for (const action of ACTIONS) {
      const button = document.createElement("button");
      button.type = "button";
      button.setAttribute("data-codex-assistant-welcome-action", action.id);
      button.setAttribute("aria-label", `${action.label}：${action.detail}`);
      const label = document.createElement("strong");
      label.replaceChildren(document.createTextNode(action.label));
      const detail = document.createElement("span");
      detail.replaceChildren(document.createTextNode(action.detail));
      button.append(label, detail);
      button.addEventListener("click", () => triggerAction(main, action));
      grid.append(button);
    }

    welcome.append(copy, grid);
    main.append(welcome);
    document.documentElement.setAttribute("data-codex-assistant-theme-home", "true");
  };

  let scheduled = false;
  const sync = (active) => {
    if (!active) {
      removeWelcome();
      return;
    }
    if (scheduled) return;
    scheduled = true;
    queueMicrotask(() => {
      scheduled = false;
      const main = firstPresent(document, MAIN_SELECTORS);
      if (!main || !isEmptyHome(main)) {
        removeWelcome();
        return;
      }
      if (!main.querySelector(WELCOME_SELECTOR)) createWelcome(main);
    });
  };

  return Object.freeze({ sync, destroy: removeWelcome });
};
