"use client";

import { useEffect } from "react";

const revealSelector = "[data-reveal]";

function setPointerPosition(element: HTMLElement, event: PointerEvent) {
  const bounds = element.getBoundingClientRect();
  element.style.setProperty("--spot-x", `${event.clientX - bounds.left}px`);
  element.style.setProperty("--spot-y", `${event.clientY - bounds.top}px`);
}

function runCounters(container: Element) {
  container.querySelectorAll<HTMLElement>("[data-count-to]").forEach((counter) => {
    if (counter.dataset.counted === "true") return;

    const target = Number(counter.dataset.countTo);
    if (!Number.isFinite(target)) return;

    counter.dataset.counted = "true";
    const duration = 720;
    let startedAt = 0;

    const tick = (time: number) => {
      if (!startedAt) startedAt = time;
      const progress = Math.min((time - startedAt) / duration, 1);
      const eased = 1 - Math.pow(1 - progress, 3);
      counter.textContent = String(Math.round(target * eased));
      if (progress < 1) window.requestAnimationFrame(tick);
    };

    counter.textContent = "0";
    window.requestAnimationFrame(tick);
  });
}

export function HomepageMotion() {
  useEffect(() => {
    const html = document.documentElement;
    const shell = document.querySelector<HTMLElement>(".site-shell");
    const header = document.querySelector<HTMLElement>(".site-header");
    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const cleanups: Array<() => void> = [];

    html.classList.add("motion-ready");

    const revealElements = Array.from(document.querySelectorAll<HTMLElement>(revealSelector));
    if (reducedMotion || !("IntersectionObserver" in window)) {
      revealElements.forEach((element) => element.classList.add("is-visible"));
    } else {
      const revealObserver = new IntersectionObserver(
        (entries) => {
          entries.forEach((entry) => {
            if (!entry.isIntersecting) return;
            entry.target.classList.add("is-visible");
            runCounters(entry.target);
            revealObserver.unobserve(entry.target);
          });
        },
        { rootMargin: "0px 0px -8%", threshold: 0.12 },
      );
      revealElements.forEach((element) => revealObserver.observe(element));
      cleanups.push(() => revealObserver.disconnect());
    }

    let scrollFrame = 0;
    const syncHeader = () => {
      scrollFrame = 0;
      header?.classList.toggle("is-scrolled", window.scrollY > 18);
    };
    const onScroll = () => {
      if (scrollFrame) return;
      scrollFrame = window.requestAnimationFrame(syncHeader);
    };
    syncHeader();
    window.addEventListener("scroll", onScroll, { passive: true });
    cleanups.push(() => {
      window.removeEventListener("scroll", onScroll);
      if (scrollFrame) window.cancelAnimationFrame(scrollFrame);
    });

    let pointerFrame = 0;
    const onWindowPointerMove = (event: PointerEvent) => {
      if (reducedMotion || !shell || pointerFrame) return;
      pointerFrame = window.requestAnimationFrame(() => {
        shell.style.setProperty("--pointer-x", `${(event.clientX / window.innerWidth) * 100}%`);
        shell.style.setProperty("--pointer-y", `${(event.clientY / window.innerHeight) * 100}%`);
        pointerFrame = 0;
      });
    };
    window.addEventListener("pointermove", onWindowPointerMove, { passive: true });
    cleanups.push(() => {
      window.removeEventListener("pointermove", onWindowPointerMove);
      if (pointerFrame) window.cancelAnimationFrame(pointerFrame);
    });

    document.querySelectorAll<HTMLElement>("[data-spotlight]").forEach((element) => {
      let frame = 0;
      const onPointerMove = (event: PointerEvent) => {
        if (frame) return;
        frame = window.requestAnimationFrame(() => {
          setPointerPosition(element, event);
          frame = 0;
        });
      };
      element.addEventListener("pointermove", onPointerMove, { passive: true });
      cleanups.push(() => {
        element.removeEventListener("pointermove", onPointerMove);
        if (frame) window.cancelAnimationFrame(frame);
      });
    });

    if (!reducedMotion) {
      document.querySelectorAll<HTMLElement>("[data-tilt]").forEach((element) => {
        const maxTilt = element.dataset.tilt === "orbit" ? 2.4 : 1.7;
        let frame = 0;
        const onPointerMove = (event: PointerEvent) => {
          if (frame) return;
          frame = window.requestAnimationFrame(() => {
            const bounds = element.getBoundingClientRect();
            const x = (event.clientX - bounds.left) / bounds.width - 0.5;
            const y = (event.clientY - bounds.top) / bounds.height - 0.5;
            element.style.setProperty("--tilt-x", `${(-y * maxTilt).toFixed(2)}deg`);
            element.style.setProperty("--tilt-y", `${(x * maxTilt).toFixed(2)}deg`);
            frame = 0;
          });
        };
        const onPointerLeave = () => {
          element.style.setProperty("--tilt-x", "0deg");
          element.style.setProperty("--tilt-y", "0deg");
        };
        element.addEventListener("pointermove", onPointerMove, { passive: true });
        element.addEventListener("pointerleave", onPointerLeave);
        cleanups.push(() => {
          element.removeEventListener("pointermove", onPointerMove);
          element.removeEventListener("pointerleave", onPointerLeave);
          if (frame) window.cancelAnimationFrame(frame);
        });
      });
    }

    const navLinks = Array.from(document.querySelectorAll<HTMLAnchorElement>(".site-header nav a"));
    const observedSections = navLinks
      .map((link) => document.querySelector<HTMLElement>(link.hash))
      .filter((section): section is HTMLElement => Boolean(section));
    if ("IntersectionObserver" in window) {
      const sectionObserver = new IntersectionObserver(
        (entries) => {
          const active = entries.find((entry) => entry.isIntersecting)?.target as HTMLElement | undefined;
          if (!active) return;
          navLinks.forEach((link) => {
            if (link.hash === `#${active.id}`) link.setAttribute("aria-current", "page");
            else link.removeAttribute("aria-current");
          });
        },
        { rootMargin: "-28% 0px -62%", threshold: 0 },
      );
      observedSections.forEach((section) => sectionObserver.observe(section));
      cleanups.push(() => sectionObserver.disconnect());
    }

    const onVisibilityChange = () => {
      document.body.classList.toggle("motion-paused", document.hidden);
    };
    document.addEventListener("visibilitychange", onVisibilityChange);
    cleanups.push(() => document.removeEventListener("visibilitychange", onVisibilityChange));

    return () => {
      cleanups.forEach((cleanup) => cleanup());
      html.classList.remove("motion-ready");
      document.body.classList.remove("motion-paused");
    };
  }, []);

  return null;
}
