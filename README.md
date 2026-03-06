<div align="center">

# ⚡ NexusVoice

**Hold a hotkey. Speak. Text appears wherever your cursor is.**

A lightweight, privacy-first voice-to-text desktop app that runs entirely on your machine — no cloud, no subscriptions, no data leaving your device.

<br/>

![Tauri](https://img.shields.io/badge/Tauri_2-24C8DB?style=for-the-badge&logo=tauri&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-CE422B?style=for-the-badge&logo=rust&logoColor=white)
![React](https://img.shields.io/badge/React_19-61DAFB?style=for-the-badge&logo=react&logoColor=black)
![TypeScript](https://img.shields.io/badge/TypeScript-3178C6?style=for-the-badge&logo=typescript&logoColor=white)
![Tailwind](https://img.shields.io/badge/Tailwind_v4-38BDF8?style=for-the-badge&logo=tailwindcss&logoColor=white)
![SQLite](https://img.shields.io/badge/SQLite-003B57?style=for-the-badge&logo=sqlite&logoColor=white)
![Whisper](https://img.shields.io/badge/Whisper_AI-412991?style=for-the-badge&logo=openai&logoColor=white)

<br/>

![Platform](https://img.shields.io/badge/Platform-Windows-0078D4?style=flat-square&logo=windows)
![License](https://img.shields.io/badge/License-MIT-green?style=flat-square)
![Version](https://img.shields.io/github/v/release/Ved-Padmawar/NexusVoice?style=flat-square&color=violet)

</div>

---

## What is NexusVoice?

NexusVoice is a push-to-talk voice transcription tool that lives in your system tray. Press your hotkey, speak, release — your words are transcribed locally by OpenAI's Whisper model and pasted directly into whatever app has focus. No internet required after the model downloads.

---

## Features

- **Push-to-talk** — hold any custom hotkey to record, release to transcribe and paste
- **100% local** — Whisper runs entirely on your machine, nothing is sent to the cloud
- **GPU-accelerated** — auto-detects NVIDIA (CUDA), AMD/Intel (DirectML), falls back to CPU
- **Smart model selection** — picks the best Whisper model for your hardware automatically
- **Auto-download** — models download on first selection, cached locally
- **Personal dictionary** — map spoken words to their correct form (e.g. "gonna" → "going to")
- **Auto-learn** — tracks uncommon words from your transcriptions and suggests additions to your dictionary
- **6 themes** — Void, Obsidian, Nord, Dusk, Sage, Paper
- **Compact pill overlay** — draggable recording indicator that stays on top while you work
- **Dashboard** — transcription history, word count, session stats
- **System tray** — runs silently in the background

---

## How It Works

```
Hotkey held      →  cpal captures mic audio
Hotkey released  →  audio resampled to 16kHz mono
                 →  Whisper model transcribes locally
                 →  text written to clipboard + Ctrl+V pasted
```

---

## Models

| Model | Size | VRAM Required | Notes |
|-------|------|--------------|-------|
| Whisper Tiny | ~75 MB | Any (CPU) | Fastest, lowest accuracy |
| Whisper Base | ~142 MB | 2 GB+ | Good for everyday use |
| Whisper Small | ~466 MB | 4 GB+ | Balanced accuracy/speed |
| Whisper Medium | ~1.5 GB | 6 GB+ | High accuracy |
| Whisper Large v3 | ~2.9 GB | 8 GB+ | Best accuracy |

Models are downloaded from HuggingFace on first selection and cached in your app data directory.

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Desktop runtime | Tauri 2 |
| Backend | Rust |
| Audio capture | cpal |
| Transcription | whisper-rs (ggml) |
| GPU inference | CUDA / DirectML |
| Database | SQLite via sqlx |
| Frontend | React 19 + TypeScript |
| Styling | Tailwind CSS v4 + shadcn/ui |
| State | Zustand |
| Icons | lucide-react |

---

## Installation

Download the latest installer from [Releases](../../releases/latest):

- **`NexusVoice_x.x.x_x64-setup.exe`** — NSIS installer (recommended)
- **`NexusVoice_x.x.x_x64_en-US.msi`** — MSI installer

**Requirements:** Windows 10 1803+ or Windows 11 (WebView2 is pre-installed).
No Rust, Node, CMake, or any dev tools needed on the target machine.

---

## Building from Source

**Prerequisites:**
- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) 18+
- [CMake](https://cmake.org/) 3.28+
- [LLVM/Clang](https://releases.llvm.org/) 17+

```bash
git clone https://github.com/Ved-Padmawar/NexusVoice.git
cd NexusVoice
npm install
npm run tauri build
```

Installer output: `src-tauri/target/release/bundle/`

**Dev server:**
```bash
npm run tauri dev
```

---

## Usage

1. Launch NexusVoice — it appears in the system tray
2. Go to **Settings → Audio** and set your recording hotkey
3. Go to **Settings → Models** and select a Whisper model (downloads automatically)
4. Click into any text field in any app
5. Hold your hotkey → speak → release
6. Your transcribed text is pasted automatically

---

## Privacy

All audio processing happens locally on your device. No audio, transcripts, or personal data is ever transmitted to any server. The only network request is the one-time model download from HuggingFace.

---

<div align="center">
  <sub>Built with ⚡ by Ved Padmawar</sub>
</div>
