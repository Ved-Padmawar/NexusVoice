import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import App from "../App";
import { useAppStore } from "../store/useAppStore";
import { invoke } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => ({
    onCloseRequested: vi.fn(() => Promise.resolve(() => {})),
    hide: vi.fn(() => Promise.resolve()),
    show: vi.fn(() => Promise.resolve()),
  })),
}));

vi.mock("@tauri-apps/plugin-updater", () => ({
  check: vi.fn(() => Promise.resolve(null)),
}));

describe("App", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "get_transcripts") return Promise.resolve([]);
      if (cmd === "get_dictionary") return Promise.resolve([]);
      // React Query rejects `undefined` as query data.
      if (cmd === "get_usage_stats")
        return Promise.resolve({
          totalWords: 0,
          speakingTimeSeconds: 0,
          totalSessions: 0,
          avgPaceWpm: 0,
        });
      return Promise.resolve(null);
    });
    useAppStore.setState({
      theme: "abyss",
      starting: false,
      // Otherwise the first-run model picker mounts over the app.
      modelChosen: true,
    });
  });

  it("renders the app shell after load", async () => {
    render(<App />);
    await waitFor(() => {
      expect(screen.getByAltText("NexusVoice")).toBeInTheDocument();
    });
  });

  it("shows the dashboard, with no sign-in step", async () => {
    render(<App />);
    await waitFor(() => {
      expect(screen.getByRole("link", { name: /dashboard/i })).toBeInTheDocument();
    });
    expect(screen.queryByText(/log in/i)).not.toBeInTheDocument();
  });
});
