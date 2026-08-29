import { describe, it, expect } from 'vitest'
import {
  formatModelSize,
  isStreaming,
  modelNameToId,
  type CatalogModel,
} from '../../lib/models'

function model(overrides: Partial<CatalogModel> = {}): CatalogModel {
  return {
    id: 'whisper-medium',
    displayName: 'Whisper Medium',
    family: 'whisper',
    pipelines: ['single-shot'],
    defaultPipeline: 'single-shot',
    sizeBytes: 582_669_056,
    multilingual: false,
    description: 'Balanced performance',
    detail: 'Great for most machines.',
    downloaded: false,
    isActive: false,
    ...overrides,
  }
}

describe('formatModelSize', () => {
  it('reports sub-gigabyte sizes in MB', () => {
    expect(formatModelSize(582_669_056)).toBe('583 MB')
  })

  it('reports gigabyte-scale sizes in GB with one decimal', () => {
    expect(formatModelSize(1_668_741_440)).toBe('1.7 GB')
  })
})

describe('isStreaming', () => {
  it('is true when streaming is among the supported paths', () => {
    expect(isStreaming(model({ pipelines: ['streaming', 'single-shot'] }))).toBe(true)
  })

  it('is false for single-shot-only models', () => {
    expect(isStreaming(model())).toBe(false)
  })
})

describe('modelNameToId', () => {
  const catalog = [model(), model({ id: 'parakeet-unified-en-0.6b', displayName: 'Parakeet Unified EN 0.6B' })]

  it('resolves a backend display name to its catalog id', () => {
    expect(modelNameToId('Parakeet Unified EN 0.6B', catalog)).toBe('parakeet-unified-en-0.6b')
  })

  it('matches case-insensitively', () => {
    expect(modelNameToId('whisper medium', catalog)).toBe('whisper-medium')
  })

  it('falls back to the first entry when nothing matches', () => {
    expect(modelNameToId('Unknown Model', catalog)).toBe('whisper-medium')
  })

  it('returns null for an empty catalog', () => {
    expect(modelNameToId('Whisper Medium', [])).toBeNull()
  })
})
