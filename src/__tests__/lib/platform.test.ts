import { describe, it, expect } from 'vitest'
import { SUPER_KEY_LABEL, isMac } from '../../lib/platform'

describe('SUPER_KEY_LABEL', () => {
  it('labels the Super modifier for the host OS', () => {
    // Label-only — the accelerator token stays `Super` on every OS.
    expect(SUPER_KEY_LABEL).toBe(isMac ? 'Cmd' : 'Win')
    expect(['Cmd', 'Win']).toContain(SUPER_KEY_LABEL)
  })
})
