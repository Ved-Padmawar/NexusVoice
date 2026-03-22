export type ModelOverride = 'large' | 'medium' | 'small'

export type ModelOption = {
  value: ModelOverride
  label: string
  description: string
  detail: string
  sizeLabel: string
}

export const MODEL_OPTIONS: ModelOption[] = [
  {
    value: 'small',
    label: 'Whisper Small',
    description: 'Fastest transcription',
    detail: 'Best for low-end CPUs or machines with less than 3 GB VRAM. Slightly lower accuracy on complex speech.',
    sizeLabel: '~150 MB',
  },
  {
    value: 'medium',
    label: 'Whisper Medium',
    description: 'Balanced accuracy & speed',
    detail: 'Great for most machines. Works well on integrated GPUs and CPUs with 8+ GB RAM.',
    sizeLabel: '~450 MB',
  },
  {
    value: 'large',
    label: 'Whisper Large v3 Turbo',
    description: 'Best accuracy',
    detail: 'Recommended for GPUs with 6+ GB VRAM or systems with 16+ GB RAM. Highest transcription quality.',
    sizeLabel: '~800 MB',
  },
]

/** Map any backend model name/display string → ModelOverride key (case-insensitive) */
export function modelNameToOverride(name: string): ModelOverride {
  const n = name.toLowerCase()
  if (n.includes('large')) return 'large'
  if (n.includes('medium')) return 'medium'
  return 'small'
}

/** Map backend recommendedModel string → ModelOverride key */
export const recommendedToOverride = modelNameToOverride
