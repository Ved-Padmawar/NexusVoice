import { describe, it, expect } from 'vitest'
import {
  formatModelSize,
  isStreaming,
  modelNameToId,
  sortForDisplay,
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
    // Must resolve a NON-first entry: the fallback returns catalog[0], so
    // asserting against the first entry passes even with matching removed.
    expect(modelNameToId('parakeet unified en 0.6b', catalog)).toBe('parakeet-unified-en-0.6b')
    expect(modelNameToId('PARAKEET UNIFIED EN 0.6B', catalog)).toBe('parakeet-unified-en-0.6b')
  })

  it('falls back to the first entry when nothing matches', () => {
    expect(modelNameToId('Unknown Model', catalog)).toBe('whisper-medium')
  })

  it('returns null for an empty catalog', () => {
    expect(modelNameToId('Whisper Medium', [])).toBeNull()
  })
})

describe('sortForDisplay', () => {
  const of = (family: string, sizeBytes: number, id = `${family}-${sizeBytes}`) =>
    model({ id, family, sizeBytes })

  it('groups families in the curated order, not alphabetically', () => {
    const sorted = sortForDisplay([of('moonshine', 1), of('whisper', 1), of('nemotron', 1)])
    expect(sorted.map(m => m.family)).toEqual(['whisper', 'nemotron', 'moonshine'])
  })

  it('orders smallest model first within a family', () => {
    const sorted = sortForDisplay([of('whisper', 900), of('whisper', 100), of('whisper', 500)])
    expect(sorted.map(m => m.sizeBytes)).toEqual([100, 500, 900])
  })

  it('sorts unlisted families after every known one', () => {
    const sorted = sortForDisplay([of('mystery', 1), of('moonshine', 1), of('whisper', 1)])
    expect(sorted.map(m => m.family)).toEqual(['whisper', 'moonshine', 'mystery'])
  })

  it('orders unlisted families alphabetically rather than by input order', () => {
    // They share a rank, so without the tiebreak the order is whatever the
    // backend happened to send — the picker would reshuffle between launches.
    const sorted = sortForDisplay([of('zeta', 1), of('alpha', 1), of('mid', 1)])
    expect(sorted.map(m => m.family)).toEqual(['alpha', 'mid', 'zeta'])
  })

  it('does not mutate the catalog it was given', () => {
    // The array comes straight from the query cache; sorting it in place would
    // reorder what every other consumer sees.
    const input = [of('moonshine', 1), of('whisper', 1)]
    const before = input.map(m => m.family)
    sortForDisplay(input)
    expect(input.map(m => m.family)).toEqual(before)
  })

  it('keeps family grouping ahead of size', () => {
    // A large whisper model still precedes a tiny moonshine one.
    const sorted = sortForDisplay([of('moonshine', 1), of('whisper', 9_000_000_000)])
    expect(sorted.map(m => m.family)).toEqual(['whisper', 'moonshine'])
  })

  it('handles an empty catalog', () => {
    expect(sortForDisplay([])).toEqual([])
  })
})
