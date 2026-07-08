<div align="center">

<img src="src-tauri/icons/128x128.png" alt="NexusVoice logo" width="96" />

# NexusVoice

**Hold a hotkey. Speak. Text appears wherever your cursor is.**

A lightweight, privacy-first voice-to-text desktop app powered by NVIDIA speech
models. Transcription runs entirely on your machine—no cloud, subscriptions, or
audio leaving your device.

<br/>

![Tauri](https://img.shields.io/badge/Tauri_2-24C8DB?style=for-the-badge&logo=tauri&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-CE422B?style=for-the-badge&logo=rust&logoColor=white)
![React](https://img.shields.io/badge/React_19-61DAFB?style=for-the-badge&logo=react&logoColor=black)
![TypeScript](https://img.shields.io/badge/TypeScript-3178C6?style=for-the-badge&logo=typescript&logoColor=white)
![NVIDIA](https://img.shields.io/badge/NVIDIA_Parakeet_%2B_Nemotron-76B900?style=for-the-badge&logo=nvidia&logoColor=white)

<br/>

![Platform](https://img.shields.io/badge/Platform-Windows_%7C_Linux-0078D4?style=flat-square)
![License](https://img.shields.io/badge/License-MIT-green?style=flat-square)
![Version](https://img.shields.io/github/v/release/Ved-Padmawar/NexusVoice?style=flat-square&color=violet)

</div>

---

## What is NexusVoice?

NexusVoice is a push-to-talk voice transcription tool that lives in your system
tray. Press your hotkey, speak, release, and the result is pasted directly into
the focused application. NVIDIA Parakeet and Nemotron models run locally through
`parakeet.cpp`; no connection is required after the selected model downloads.

---

## Features

- **Push-to-talk** — hold any custom hotkey to record, then release to transcribe
- **Dictation mode** — start, pause, resume, and commit longer recordings
- **100% local audio processing** — microphone audio never leaves the machine
- **GPU acceleration** — Vulkan standard builds, a Windows CUDA build, and CPU fallback
- **Five NVIDIA model tiers** — from a 110M lightweight model to a 1.1B accuracy model
- **Hardware-aware recommendation** — sensible default based on available RAM and VRAM
- **Personal dictionary** — map spoken terms to their preferred written form
- **Smart formatting (optional)** — use any OpenAI-compatible LLM endpoint
- **Eight themes** — four dark and four light visual themes
- **Compact pill overlay** — an always-on-top recording and processing indicator
- **Local dashboard** — searchable transcript history and usage statistics

---

## How It Works

```text
App launch       → selected GGUF model loaded once through parakeet.cpp
Hotkey held      → cpal captures microphone audio
Hotkey released  → denoise → resample to 16 kHz → VAD → normalize
                 → Parakeet or Nemotron transcribes direct float PCM
                 → personal dictionary corrections applied
                 → optional smart formatting applied
                 → text copied and pasted into the focused application
```

The generic audio capture and preprocessing stages remain shared across every
model. Model decoding, language prompting, and CPU/GPU execution are handled by
the pinned `parakeet.cpp` runtime.

---

## Models

| Tier | NVIDIA model | Download | Best for |
|---|---|---:|---|
| Tiny | Parakeet TDT-CTC 110M | 143 MB | Fastest startup and low-resource machines |
| Small | Parakeet Realtime EOU 120M | 141 MB | Endpoint-aware streaming workloads |
| Medium | Parakeet TDT 0.6B v3 | 742 MB | Accurate multilingual dictation |
| Turbo | Nemotron 3.5 ASR 0.6B | 785 MB | Cache-aware streaming across 40+ locales |
| Large | Parakeet TDT 1.1B | 1.21 GB | Maximum English transcription accuracy |

The application uses upstream Q5_K GGUF artifacts from
[`mudler/parakeet-cpp-gguf`](https://huggingface.co/mudler/parakeet-cpp-gguf),
downloaded on demand and cached locally. Models can be changed or removed from
Settings at any time.

---

## Smart Formatting (optional)

By default, NexusVoice pastes the raw local transcription. Smart Formatting can
clean punctuation and infer structure through an OpenAI-compatible chat endpoint.
It supports local Ollama or LM Studio servers as well as cloud providers. If the
formatter is unavailable, NexusVoice safely falls back to the raw transcript.

---

## Tech Stack

| Layer | Technology |
|---|---|
| Desktop runtime | Tauri 2 |
| Backend | Rust |
| Audio capture | cpal |
| Speech runtime | parakeet.cpp + GGML |
| Models | NVIDIA Parakeet and Nemotron ASR |
| GPU inference | Vulkan / CUDA, with CPU fallback |
| Audio preprocessing | RNNoise, VAD, rubato resampling |
| Database | SQLite via sqlx |
| Frontend | React 19 + TypeScript + Tailwind CSS v4 |
| State | Zustand |

---

## Installation

Download the latest Windows or Linux build from [Releases](../../releases/latest).

- **Standard build:** Vulkan acceleration across supported NVIDIA, AMD, and Intel GPUs
- **Windows CUDA build:** optimized NVIDIA acceleration
- **Fallback:** every build retains portable CPU execution

Windows requires WebView2. Linux requires WebKitGTK 4.1, `libxdo3`, and a Vulkan
driver for GPU acceleration. No development tools are needed on target machines.

---

## Building from Source

Prerequisites: stable Rust, Node.js 24+, CMake 3.18+, and the platform packages
required by Tauri.

```bash
git clone --recursive https://github.com/Ved-Padmawar/NexusVoice.git
cd NexusVoice
npm install
npm run tauri dev
```

Development builds compile a portable CPU `parakeet.cpp` runtime by default.
Set `NEXUSVOICE_PARAKEET_BACKEND=vulkan` or `cuda` before building to test a GPU
backend. Release automation builds and bundles the correct native library.

---

## Usage

1. Launch NexusVoice and sign in to the local profile.
2. Choose an NVIDIA speech model; the hardware recommendation is preselected.
3. Configure push-to-talk and optional dictation hotkeys.
4. Focus any text field, hold the recording hotkey, speak, and release.
5. The local transcription is pasted automatically.

---

## Privacy

All audio capture, preprocessing, and speech recognition happen locally. The only
default network operation is downloading a selected model. Smart Formatting is
optional and disabled by default; when enabled, only transcript text is sent to
the configured endpoint. Point it at a local server to keep that stage offline.

---

<div align="center">
  <sub>Built with ⚡ by Ved Padmawar</sub>
</div>
