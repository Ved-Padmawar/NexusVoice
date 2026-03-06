import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import App from "./App";
import { useAppStore } from "./store/useAppStore";
import { invoke } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => ({
    onCloseRequested: vi.fn(() => Promise.resolve(() => {})),
    hide: vi.fn(() => Promise.resolve()),
  })),
}));

describe("App", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "get_transcripts") return Promise.resolve([]);
      if (cmd === "get_dictionary") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
    useAppStore.setState({
      transcripts: [],
      dictionary: [],
      theme: "void",
      isLoading: false,
    });
  });

  it("renders NexusVoice title after load", async () => {
    render(<App />);
    await waitFor(() => {
      expect(screen.getByText(/NexusVoice/i)).toBeInTheDocument();
    });
  });
});
