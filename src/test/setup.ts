import "@testing-library/jest-dom/vitest";

// jsdom has no layout engine. Give elements a size so the virtualised list renders rows.
Object.defineProperties(HTMLElement.prototype, {
  clientWidth: { configurable: true, get: () => 900 },
  clientHeight: { configurable: true, get: () => 600 },
  offsetWidth: { configurable: true, get: () => 900 },
  offsetHeight: { configurable: true, get: () => 600 },
});
Element.prototype.getBoundingClientRect = function () {
  return { x: 0, y: 0, top: 0, left: 0, bottom: 600, right: 900, width: 900, height: 600, toJSON: () => ({}) } as DOMRect;
};
Element.prototype.scrollTo = function () {};

class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
globalThis.ResizeObserver ??= ResizeObserverStub as unknown as typeof ResizeObserver;
