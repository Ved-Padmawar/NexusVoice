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

![Platform](https://img.shields.io/badge/Platform-Windows_%7C_Linux-0078D4?style=flat-square)
![License](https://img.shields.io/badge/License-MIT-green?style=flat-square)
![Version](https://img.shields.io/badge/Version-v1.12.3-violet?style=flat-square)

</div>

---

## What is NexusVoice?

NexusVoice is a push-to-talk voice transcription tool that lives in your system tray. Press your hotkey, speak, release — your words are transcribed locally by OpenAI's Whisper model and pasted directly into whatever app has focus. No internet required after the model downloads.

---

## Features

- **Push-to-talk** — hold any custom hotkey to record, release to transcribe and paste
- **Live streaming transcription** — your speech is transcribed as you talk, so there's barely anything left to process when you release the hotkey
- **Dictation Mode** — a hands-free alternative: press a hotkey to start, then pause/resume and save from the pill or by hotkey — ideal for longer, uninterrupted dictation
- **Microphone selection** — pick which input device records your voice (Settings → Shortcuts); defaults to the system default and falls back to it automatically if your chosen mic is unplugged
- **100% local** — Whisper runs entirely on your machine, nothing is sent to the cloud
- **GPU-accelerated** — auto-detects NVIDIA (CUDA), AMD/Intel (Vulkan), falls back to CPU
- **Smart model selection** — picks the best Whisper model for your hardware automatically
- **First-run model picker** — choose your model on first login with a hardware-aware recommendation, then download on demand
- **Personal dictionary** — map spoken words to their correct form (e.g. "gonna" → "going to")
- **Smart formatting (optional)** — clean up punctuation and turn spoken lists into real lists using any OpenAI-compatible LLM. Off by default; works fully local with Ollama or LM Studio, or with a cloud provider (OpenAI, OpenRouter) if you prefer
- **8 themes** — Abyss, Midnight, Steel, Pine (dark) + Canvas, Dawn, Breeze, Blossom (light)
- **Compact pill overlay** — draggable recording indicator that stays on top while you work
- **Dashboard** — transcription history, word count, session stats
- **System tray** — runs silently in the background

---

## How It Works

```
App launch       →  Whisper model loaded and warmed up in the background
Hotkey held      →  cpal captures mic audio from your selected input device
                    (capture starts before "recording" is reported, so the
                    first words aren't clipped)
While speaking   →  audio is transcribed continuously in the background; text
                    only becomes final once two consecutive passes agree on it
                 →  silent lead-in is trimmed and the audio is denoised and
                    level-normalised before it reaches Whisper
Hotkey released  →  a brief post-roll captures trailing speech, then only the
                    undecoded tail is transcribed
                 →  (optional) transcript reformatted by your chosen LLM
                 →  text written to clipboard + Ctrl+V pasted
```

Transcription runs while you speak rather than waiting until you finish, so releasing the hotkey only leaves the last few seconds to process — the longer you talk, the bigger the saving. Each pass re-reads the recent audio in full sentence context, and a word is only committed once two consecutive passes agree on it, so accuracy matches decoding everything at the end. Committed text is never revised, so what you see never rewrites itself.

The engine pre-loads at launch so the first transcription is instant. If it isn't ready yet, or the clip is very short, the whole recording is simply decoded in one pass on release.

If **Smart Formatting** is enabled, the transcript is sent to your configured LLM endpoint for cleanup before pasting; otherwise the raw transcript is pasted as-is. If the formatter is unreachable or errors, it transparently falls back to the raw transcript.

---

## Models

| Model                  |   Size   | Used When                       | Notes                               |
|------------------------|:--------:|---------------------------------|-------------------------------------|
| Whisper Large v3       | 2.9&nbsp;GB | GPU with 6GB+ VRAM              | Maximum accuracy, full multilingual |
| Whisper Large v3 Turbo | 834&nbsp;MB | GPU with 4GB+ VRAM or 12GB+ RAM | Best accuracy, fast on GPU          |
| Whisper Medium         | 514&nbsp;MB | Mid-range GPU or 6GB+ RAM       | Great accuracy, runs well on CPU    |
| Whisper Small          | 181&nbsp;MB | Moderate hardware or 2GB+ VRAM  | Good balance of speed and quality   |
| Whisper Base           | 57&nbsp;MB  | Low-end hardware                | Basic accuracy, fast inference      |
| Whisper Tiny           | 31&nbsp;MB  | Ultra-low-end hardware          | Fastest, lowest accuracy            |

Models are quantized for smaller downloads and faster inference at near-identical accuracy (the full multilingual Large v3 is kept at full precision as the accuracy anchor). On first login a model picker modal lets you choose your model — the app recommends the best one for your hardware. You can change it anytime in Settings → About, where you can also delete downloaded models. Models download from HuggingFace and are cached locally.

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

Download the latest build for your platform from [Releases](../../releases/latest):

**Windows**

| Installer | Who it's for |
|-----------|-------------|
| `NexusVoice_x.x.x_x64-setup.exe` | Everyone — CPU + Vulkan (Intel, AMD, NVIDIA) |
| `NexusVoice-CUDA_x.x.x_x64-setup.exe` | NVIDIA GPU users who want maximum performance |

**Linux** (`.deb` for Debian/Ubuntu/Zorin; `.rpm` for Fedora/RHEL)

| Build | Who it's for |
|-------|-------------|
| `NexusVoice_x.x.x_amd64.deb` / `.rpm` | Everyone — CPU + Vulkan (Intel, AMD, **and NVIDIA**) |

On Linux, Vulkan GPU-accelerates NVIDIA, AMD, and Intel alike, so a single build covers every GPU — no separate CUDA download needed.

**Requirements:**
- **Windows:** Windows 10 1803+ or Windows 11 (WebView2 is pre-installed).
- **Linux:** WebKitGTK 4.1 (`libwebkit2gtk-4.1`) and `libxdo3`, both pulled in automatically by the `.deb`/`.rpm`. For GPU acceleration, install your distro's Vulkan driver (Mesa for AMD/Intel, the NVIDIA driver for NVIDIA).

No Rust, Node, CMake, or any dev tools needed on the target machine.

> _macOS support is implemented in the codebase but not yet distributed — macOS builds require Apple code-signing/notarization, which is planned for a future release._

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
2. Go to **Settings → Shortcuts** and set your recording hotkey (and, optionally, a separate Dictation hotkey)
3. Choose your Whisper model in the first-run picker — the recommended one is pre-selected for your hardware
4. Click into any text field in any app
5. **Push-to-talk:** hold your hotkey → speak → release
6. **Dictation Mode:** press your dictation hotkey to start, pause/resume as needed, then save from the pill or the commit hotkey
7. Your transcribed text is pasted automatically

---

## Privacy

All **audio processing and transcription happen locally** on your device — audio is never transmitted anywhere. The only network request in the default setup is the one-time Whisper model download from HuggingFace.

**Smart Formatting** is the one optional exception, and it's **off by default**. When enabled, your transcript text (never the audio) is sent to whichever LLM endpoint you configure. Point it at a local server (Ollama or LM Studio) to keep everything on-device, or at a cloud provider (OpenAI, OpenRouter) if you choose — in which case the transcript text is sent to that provider. The choice, and whether to enable it at all, is entirely yours.

---

<div align="center">
  <sub>Built with ⚡ by Ved Padmawar</sub>
</div>
