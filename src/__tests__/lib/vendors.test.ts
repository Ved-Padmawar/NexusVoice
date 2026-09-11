import { describe, it, expect } from 'vitest'
import { VENDORS, RASTER, isRaster, vendorForFamily } from '../../lib/vendors'
// The real backend catalog, imported rather than read, so no node APIs needed.
import catalogJson from '../../../src-tauri/src/inference/models.json'

describe('vendorForFamily', () => {
  it('resolves a plain family name', () => {
    expect(vendorForFamily('whisper')).toBe('openai')
    expect(vendorForFamily('parakeet')).toBe('nvidia')
    expect(vendorForFamily('moonshine')).toBe('moonshine')
  })

  it('strips hyphens before looking the family up', () => {
    // The catalog ships `qwen3-asr`; the table is keyed `qwen3asr`. Without the
    // normalisation the logo silently disappears — no error, just a blank slot.
    expect(vendorForFamily('qwen3-asr')).toBe('qwen')
  })

  it('returns null for an unknown family instead of guessing', () => {
    expect(vendorForFamily('not-a-family')).toBeNull()
    expect(vendorForFamily('')).toBeNull()
  })

  it('every mapped vendor actually has a logo registered', () => {
    // A typo'd vendor id is invisible at build time and renders nothing.
    for (const family of ['whisper', 'parakeet', 'nemotron', 'canary', 'qwen3-asr', 'moonshine']) {
      const id = vendorForFamily(family)
      expect(id, `${family} maps to nothing`).not.toBeNull()
      const known = id! in VENDORS || id! in RASTER
      expect(known, `${family} -> "${id}" is not in VENDORS or RASTER`).toBe(true)
    }
  })

  it('resolves every family the backend catalog ships', () => {
    // The real cross-boundary check: a family added to models.json with no
    // vendor mapping ships a model card with a missing logo.
    const { models } = catalogJson

    expect(models.length).toBeGreaterThan(0)
    for (const m of models) {
      expect(vendorForFamily(m.family), `${m.id}: family "${m.family}" has no vendor logo`).not.toBeNull()
    }
  })
})

describe('vendor registry', () => {
  it('separates vector marks from raster ones', () => {
    // `isRaster` decides between <svg> and <img>; a mark in the wrong bucket
    // renders as a broken element.
    expect(isRaster('moonshine')).toBe(true)
    for (const id of Object.keys(VENDORS)) expect(isRaster(id)).toBe(false)
  })

  it('gives every vector vendor a renderable mark', () => {
    for (const [id, v] of Object.entries(VENDORS)) {
      expect(v.Mark, `${id} has no Mark`).toBeTruthy()
    }
  })

  it('uses a null colour only for marks that carry their own', () => {
    // A null colour means "do not tint"; a hex means the file is monochrome.
    // Getting this backwards renders an invisible or wrongly-tinted logo.
    for (const [id, v] of Object.entries(VENDORS)) {
      if (v.color !== null) {
        expect(v.color, `${id} colour must be a hex string`).toMatch(/^#[0-9A-Fa-f]{6}$/)
      }
    }
    expect(VENDORS.anthropic.color).toBeNull()
    expect(VENDORS.openai.color).toBe('#10A37F')
  })

  it('no vendor id collides between the vector and raster registries', () => {
    for (const id of Object.keys(RASTER)) expect(id in VENDORS).toBe(false)
  })
})
