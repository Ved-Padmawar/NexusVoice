<div align="center">

<img src="src-tauri/icons/128x128.png" alt="NexusVoice logo" width="96" />

# NexusVoice

**Hold a hotkey. Speak. Text appears wherever your cursor is.**

A lightweight, privacy-first voice-to-text desktop app. Transcription runs entirely on your machine — no cloud, no subscriptions, no data leaving your device.

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
- **Low-latency streaming** — audio is processed in chunks mid-recording so only the tail needs processing on release
- **100% local** — Whisper runs entirely on your machine, nothing is sent to the cloud
- **GPU-accelerated** — auto-detects NVIDIA (CUDA), AMD/Intel (Vulkan), falls back to CPU
- **Smart model selection** — picks the best Whisper model for your hardware automatically
- **First-run model picker** — choose your model on first login with a hardware-aware recommendation, then download on demand
- **Personal dictionary** — map spoken words to their correct form (e.g. "gonna" → "going to")
- **Auto-learn** — tracks uncommon words from your transcriptions and suggests additions to your dictionary
- **Smart formatting (optional)** — clean up punctuation and turn spoken lists into real lists using any OpenAI-compatible LLM. Off by default; works fully local with Ollama or LM Studio, or with a cloud provider (OpenAI, OpenRouter) if you prefer
- **8 themes** — Abyss, Midnight, Steel, Pine (dark) + Canvas, Dawn, Breeze, Blossom (light)
- **Compact pill overlay** — draggable recording indicator that stays on top while you work
- **Dashboard** — transcription history, word count, session stats
- **System tray** — runs silently in the background

---

## How It Works

```
App launch       →  Whisper model loaded and warmed up in the background
Hotkey held      →  cpal captures mic audio
                 →  VAD-gated chunks processed mid-recording (every ~8s)
                 →  silence boundaries detected to avoid cutting words
Hotkey released  →  only the final tail segment (~last 6s) is transcribed
                 →  chunks stitched together with overlap deduplication
                 →  (optional) transcript reformatted by your chosen LLM
                 →  text written to clipboard + Ctrl+V pasted
```

For short recordings the pipeline is transparent — everything processes on release as before. For longer recordings latency is significantly reduced since most of the audio is already transcribed by the time you let go of the hotkey. The engine pre-loads at launch so the first transcription is instant.

If **Smart Formatting** is enabled, the stitched transcript is sent to your configured LLM endpoint for cleanup before pasting; otherwise the raw transcript is pasted as-is. If the formatter is unreachable or errors, it transparently falls back to the raw transcript.

---

## Models

| Model | Size | Used When | Notes |
|-------|------|-----------|-------|
| Whisper Large v3 Turbo | ~1.6 GB | GPU with 6GB+ VRAM or 16GB+ RAM | Best accuracy, fast on GPU |
| Whisper Medium | ~1.5 GB | Mid-range GPU or 8GB+ RAM | Great accuracy, runs well on CPU |
| Whisper Small | ~465 MB | Moderate hardware or 4GB+ RAM | Good balance of speed and quality |
| Whisper Base | ~145 MB | Low-end hardware | Basic accuracy, fast inference |
| Whisper Tiny | ~75 MB | Ultra-low-end hardware | Fastest, lowest accuracy |

On first login a model picker modal lets you choose your model — the app recommends the best one for your hardware. You can change it anytime in Settings → About. Models download from HuggingFace and are cached locally.

---

## Smart Formatting (optional)

By default NexusVoice pastes the raw transcription. Enable **Smart Formatting** (Settings → About) to have an LLM clean up punctuation and infer structure — e.g. turning a spoken "first… second… third…" into a real numbered list — before pasting.

It connects to any **OpenAI-compatible** chat endpoint. Presets prefill the base URL; you supply the model name and (where needed) an API key:

| Provider | Local? | Notes |
|----------|--------|-------|
| Ollama | ✅ | `http://localhost:11434/v1`, no key needed |
| LM Studio | ✅ | `http://localhost:1234/v1`, no key needed |
| OpenAI | ☁️ | requires API key |
| OpenRouter | ☁️ | requires API key |
| Custom | — | any other OpenAI-compatible endpoint |

Use a small **instruct** model (e.g. `qwen2.5-3b-instruct`) — not a reasoning/thinking model — for the best speed and most faithful formatting. The feature is off by default; with Ollama or LM Studio it stays fully on-device.

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Desktop runtime | Tauri 2 |
| Backend | Rust |
| Audio capture | cpal |
| Transcription | whisper-rs (ggml) |
| GPU inference | CUDA (NVIDIA) / Vulkan (AMD, Intel) |
| Smart formatting | Any OpenAI-compatible LLM (Ollama, LM Studio, OpenAI, OpenRouter) over HTTP |
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
3. Choose your Whisper model in the first-run picker — the recommended one is pre-selected for your hardware
4. Click into any text field in any app
5. Hold your hotkey → speak → release
6. Your transcribed text is pasted automatically

---

## Privacy

All **audio processing and transcription happen locally** on your device — audio is never transmitted anywhere. The only network request in the default setup is the one-time Whisper model download from HuggingFace.

**Smart Formatting** is the one optional exception, and it's **off by default**. When enabled, your transcript text (never the audio) is sent to whichever LLM endpoint you configure. Point it at a local server (Ollama or LM Studio) to keep everything on-device, or at a cloud provider (OpenAI, OpenRouter) if you choose — in which case the transcript text is sent to that provider. The choice, and whether to enable it at all, is entirely yours.

---

<div align="center">
  <sub>Built with ⚡ by Ved Padmawar</sub>
</div>
