export type ModelOverride = 'tiny' | 'base' | 'small' | 'medium' | 'large'

export type ModelOption = {
  value: ModelOverride
  label: string
  description: string
  detail: string
  sizeLabel: string
}

export const MODEL_OPTIONS: ModelOption[] = [
  {
    value: 'tiny',
    label: 'Whisper Tiny',
    description: 'Fastest, lowest accuracy',
    detail: 'Ultra-lightweight model for very low-end hardware. Best when speed matters more than accuracy.',
    sizeLabel: '~75 MB',
  },
  {
    value: 'base',
    label: 'Whisper Base',
    description: 'Fast, basic accuracy',
    detail: 'Lightweight model for low-end hardware. Good for simple dictation where speed is preferred.',
    sizeLabel: '~145 MB',
  },
  {
    value: 'small',
    label: 'Whisper Small',
    description: 'Standard, lower accuracy',
    detail: 'Best for low-end CPUs or machines with less than 3 GB VRAM. Good balance of speed and quality.',
    sizeLabel: '~465 MB',
  },
  {
    value: 'medium',
    label: 'Whisper Medium',
    description: 'Balanced performance',
    detail: 'Great for most machines. Works well on integrated GPUs and CPUs with 8+ GB RAM.',
    sizeLabel: '~1.5 GB',
  },
  {
    value: 'large',
    label: 'Whisper Large v3 Turbo',
    description: 'Slowest, highest accuracy',
    detail: 'Recommended for GPUs with 6+ GB VRAM or systems with 16+ GB RAM. Highest transcription quality.',
    sizeLabel: '~1.6 GB',
  },
]

/** Map any backend model name/display string → ModelOverride key (case-insensitive) */
export function modelNameToOverride(name: string): ModelOverride {
  const n = name.toLowerCase()
  if (n.includes('large')) return 'large'
  if (n.includes('medium')) return 'medium'
  if (n.includes('small')) return 'small'
  if (n.includes('base')) return 'base'
  return 'tiny'
}

/** Map backend recommendedModel string → ModelOverride key */
export const recommendedToOverride = modelNameToOverride
