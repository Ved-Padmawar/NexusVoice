import { vi } from "vitest";
import "@testing-library/jest-dom/vitest";

// Mock Tauri invoke for tests (no backend in Vitest)
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));
