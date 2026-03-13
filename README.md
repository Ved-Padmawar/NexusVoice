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
- **GPU-accelerated** — auto-detects NVIDIA (CUDA), AMD/Intel (Vulkan), falls back to CPU
- **Smart model selection** — picks the best Whisper model for your hardware automatically
- **Auto-download** — models download on first login, cached locally
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

| Model | Size | Used When | Notes |
|-------|------|-----------|-------|
| ggml-large-v3-turbo | ~1.5 GB | GPU detected | Best accuracy, fast on GPU |
| ggml-medium.en | ~750 MB | CPU only | Great accuracy, runs well on CPU |

The app auto-selects the best model for your hardware. You can override in Settings → Models. Models are downloaded from HuggingFace on first login and cached locally.

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Desktop runtime | Tauri 2 |
| Backend | Rust |
| Audio capture | cpal |
| Transcription | whisper-rs (ggml) |
| GPU inference | CUDA (NVIDIA) / Vulkan (AMD, Intel) |
| Database | SQLite via sqlx |
| Frontend | React 19 + TypeScript |
| Styling | Tailwind CSS v4 + shadcn/ui |
| State | Zustand |
| Icons | lucide-react |

---

## Installation

Download the latest installer from [Releases](../../releases/latest):

| Installer | Who it's for |
|-----------|-------------|
| `NexusVoice_x.x.x_x64-setup.exe` | Everyone — CPU + Vulkan (Intel, AMD, NVIDIA) |
| `NexusVoice-CUDA_x.x.x_x64-setup.exe` | NVIDIA GPU users who want maximum performance |

If you're unsure, download the standard installer — it works on all machines.

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
3. Go to **Settings → About** to verify model status (downloads automatically on login)
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
