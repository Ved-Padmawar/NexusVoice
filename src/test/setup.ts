import { vi } from "vitest";
import "@testing-library/jest-dom/vitest";

// Mock Tauri APIs for tests (no backend in Vitest)
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

// jsdom doesn't implement these; Select scrolls its options into view and
// Popover re-places itself when its panel resizes.
if (!Element.prototype.scrollIntoView) {
  Element.prototype.scrollIntoView = () => {};
}
if (!globalThis.ResizeObserver) {
  globalThis.ResizeObserver = class {
    observe() {}
    unobserve() {}
    disconnect() {}
  };
}

// jsdom declares scrollTo but throws when called, so assign unconditionally.
window.scrollTo = () => {};
