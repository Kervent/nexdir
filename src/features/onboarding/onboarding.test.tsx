import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { loadOnboarding, recordTour, shouldOfferTour, TOUR_STEPS, TOUR_VERSION, type TourStep } from "./onboarding";
import { placeCard, Tour } from "./Tour";

beforeEach(() => localStorage.clear());

describe("tour state", () => {
  it("offers the tour until it is skipped or completed", () => {
    expect(shouldOfferTour(loadOnboarding())).toBe(true);
    recordTour("offered");
    expect(shouldOfferTour(loadOnboarding())).toBe(true); // closed without choosing
    recordTour("skipped");
    expect(shouldOfferTour(loadOnboarding())).toBe(false);
    recordTour("completed");
    expect(loadOnboarding()).toMatchObject({ version: TOUR_VERSION, status: "completed" });
    expect(shouldOfferTour(loadOnboarding())).toBe(false);
  });

  it("offers a new tour version without touching preferences", () => {
    localStorage.setItem("nexdir.preferences.v1", JSON.stringify({ theme: "dark" }));
    recordTour("completed", new Date(), 1);
    expect(shouldOfferTour(loadOnboarding(), 1)).toBe(false);
    expect(shouldOfferTour(loadOnboarding(), 2)).toBe(true);
    expect(localStorage.getItem("nexdir.preferences.v1")).toBe(JSON.stringify({ theme: "dark" }));
  });

  it("keeps the first offer time and records the decision time", () => {
    recordTour("offered", new Date("2026-11-01T10:00:00Z"));
    const s = recordTour("skipped", new Date("2026-11-01T10:05:00Z"));
    expect(s.offeredAt).toBe("2026-11-01T10:00:00.000Z");
    expect(s.decidedAt).toBe("2026-11-01T10:05:00.000Z");
  });

  it("ignores damaged stored state", () => {
    localStorage.setItem("nexdir.onboarding.v1", "{not json");
    expect(loadOnboarding()).toBeNull();
    localStorage.setItem("nexdir.onboarding.v1", JSON.stringify({ version: 1, status: "weird" }));
    expect(loadOnboarding()).toBeNull();
  });

  it("has the five steps of the introduction", () => {
    expect(TOUR_STEPS.map((s) => s.id)).toEqual(["navigation", "tabs", "files", "tools", "personalize"]);
  });
});

describe("card placement", () => {
  it("never leaves the window", () => {
    const sizes: [number, number][] = [
      [1240, 780],
      [720, 460],
      [400, 300],
    ];
    for (const [vw, vh] of sizes) {
      for (const anchor of [null, { top: 0, left: 0, width: 50, height: 50 }, { top: vh - 20, left: vw - 20, width: 20, height: 20 }, { top: 0, left: 0, width: vw, height: vh }]) {
        const p = placeCard(anchor, 360, 280, vw, vh);
        expect(p.left).toBeGreaterThanOrEqual(0);
        expect(p.top).toBeGreaterThanOrEqual(0);
        if (vw >= 384) expect(p.left + 360).toBeLessThanOrEqual(vw);
        if (vh >= 304) expect(p.top + 280).toBeLessThanOrEqual(vh);
      }
    }
  });

  it("prefers the right side, then below", () => {
    expect(placeCard({ top: 100, left: 10, width: 200, height: 400 }, 300, 200, 1200, 800)).toEqual({ left: 224, top: 100 });
    expect(placeCard({ top: 10, left: 0, width: 1200, height: 40 }, 300, 200, 1200, 800)).toEqual({ left: 12, top: 64 });
  });
});

const steps: TourStep[] = [
  { id: "a", title: "First step", body: ["Alpha"], targets: ["one"] },
  { id: "b", title: "Second step", body: ["Beta"], targets: ["missing"] },
];

describe("Tour", () => {
  let target: HTMLElement;
  beforeEach(() => {
    target = document.createElement("div");
    target.dataset.tour = "one";
    document.body.appendChild(target);
  });
  afterEach(() => target.remove());

  it("walks forward and back, then completes", () => {
    const onFinish = vi.fn();
    render(<Tour steps={steps} onFinish={onFinish} />);
    expect(screen.getByRole("dialog", { name: "First step" })).toBeInTheDocument();
    expect(screen.getByText("Step 1 of 2")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Back/ })).toBeDisabled();
    expect(document.querySelectorAll(".nx-tour-ring")).toHaveLength(1);

    fireEvent.click(screen.getByRole("button", { name: /Next/ }));
    expect(screen.getByRole("dialog", { name: "Second step" })).toBeInTheDocument();
    // The element of this step does not exist: no highlight, a hint instead, no crash.
    expect(document.querySelectorAll(".nx-tour-ring")).toHaveLength(0);
    expect(screen.getByText(/not visible right now/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Back/ }));
    expect(screen.getByRole("dialog", { name: "First step" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /Next/ }));
    fireEvent.click(screen.getByRole("button", { name: /Finish/ }));
    expect(screen.getByRole("dialog", { name: "You're ready to explore Nexdir!" })).toBeInTheDocument();
    expect(onFinish).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Get Started" }));
    expect(onFinish).toHaveBeenCalledWith(true);
  });

  it("works with the keyboard and moves focus to the main button", () => {
    const onFinish = vi.fn();
    render(<Tour steps={steps} onFinish={onFinish} />);
    expect(screen.getByRole("button", { name: /Next/ })).toHaveFocus();
    const card = screen.getByRole("dialog");
    fireEvent.keyDown(card, { key: "ArrowRight" });
    expect(screen.getByRole("dialog", { name: "Second step" })).toBeInTheDocument();
    fireEvent.keyDown(card, { key: "ArrowLeft" });
    expect(screen.getByRole("dialog", { name: "First step" })).toBeInTheDocument();
    // Tab wraps inside the card.
    const skip = screen.getByRole("button", { name: "Skip tour" });
    const next = screen.getByRole("button", { name: /Next/ });
    next.focus();
    fireEvent.keyDown(next, { key: "Tab" });
    expect(skip).toHaveFocus();
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(onFinish).toHaveBeenCalledWith(false);
  });

  it("takes focus from elsewhere and handles keys even when focus left the card", () => {
    const outside = document.createElement("button");
    document.body.appendChild(outside);
    outside.focus();
    const onFinish = vi.fn();
    render(<Tour steps={steps} onFinish={onFinish} />);
    expect(screen.getByRole("button", { name: /Next/ })).toHaveFocus();
    outside.focus();
    fireEvent.keyDown(outside, { key: "ArrowRight" });
    expect(screen.getByRole("dialog", { name: "Second step" })).toBeInTheDocument();
    fireEvent.keyDown(document.body, { key: "Escape" });
    expect(onFinish).toHaveBeenCalledWith(false);
    outside.remove();
  });

  it("can be skipped at any step", () => {
    const onFinish = vi.fn();
    render(<Tour steps={steps} onFinish={onFinish} />);
    fireEvent.click(screen.getByRole("button", { name: /Next/ }));
    fireEvent.click(screen.getByRole("button", { name: "Skip tour" }));
    expect(onFinish).toHaveBeenCalledWith(false);
  });

  it("re-measures when the window is resized and hides highlights of hidden elements", () => {
    render(<Tour steps={steps} onFinish={() => {}} />);
    expect(document.querySelectorAll(".nx-tour-ring")).toHaveLength(1);
    target.hidden = true;
    act(() => {
      window.dispatchEvent(new Event("resize"));
    });
    expect(document.querySelectorAll(".nx-tour-ring")).toHaveLength(0);
  });

  it("announces the step for screen readers and shows progress", () => {
    render(<Tour steps={steps} onFinish={() => {}} />);
    expect(screen.getByText("Step 1 of 2: First step")).toHaveAttribute("aria-live", "polite");
    expect(screen.getByRole("progressbar", { name: "Tour progress" })).toHaveAttribute("aria-valuenow", "1");
  });
});
