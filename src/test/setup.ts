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
