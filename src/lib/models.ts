export type ModelVariant =
  | 'parakeet-tdt-ctc-110m'
  | 'parakeet-realtime-eou-120m'
  | 'parakeet-tdt-0.6b-v3'
  | 'nemotron-3.5-asr-0.6b'
  | 'parakeet-tdt-1.1b'

export type ModelOption = {
  value: ModelVariant
  tier: 'Tiny' | 'Small' | 'Medium' | 'Turbo' | 'Large'
  label: string
  description: string
  detail: string
  sizeLabel: string
}

export const MODEL_OPTIONS: ModelOption[] = [
  { value: 'parakeet-tdt-ctc-110m', tier: 'Tiny', label: 'Parakeet TDT-CTC 110M', description: 'Fastest, lowest resource use', detail: 'Compact NVIDIA Parakeet model for low-latency transcription on modest hardware.', sizeLabel: '~143 MB' },
  { value: 'parakeet-realtime-eou-120m', tier: 'Small', label: 'Parakeet Realtime EOU 120M', description: 'Realtime speech with endpoint detection', detail: 'Streaming-focused Parakeet model with end-of-utterance detection for responsive dictation.', sizeLabel: '~141 MB' },
  { value: 'parakeet-tdt-0.6b-v3', tier: 'Medium', label: 'Parakeet TDT 0.6B v3', description: 'Balanced speed and accuracy', detail: 'The balanced multilingual Parakeet model for everyday transcription.', sizeLabel: '~742 MB' },
  { value: 'nemotron-3.5-asr-0.6b', tier: 'Turbo', label: 'Nemotron 3.5 ASR 0.6B', description: 'Streaming, multilingual transcription', detail: 'Cache-aware NVIDIA Nemotron streaming ASR for responsive multilingual use.', sizeLabel: '~785 MB' },
  { value: 'parakeet-tdt-1.1b', tier: 'Large', label: 'Parakeet TDT 1.1B', description: 'Maximum transcription quality', detail: 'The largest Parakeet option for accuracy-first transcription on capable hardware.', sizeLabel: '~1.2 GB' },
]

export function modelNameToVariant(name: string): ModelVariant {
  const normalized = name.toLowerCase()
  return MODEL_OPTIONS.find(({ value, label }) => normalized.includes(value) || normalized.includes(label.toLowerCase()))?.value
    ?? 'parakeet-tdt-0.6b-v3'
}

export const recommendedToVariant = modelNameToVariant
