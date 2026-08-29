import { describe, it, expect } from 'vitest'
import { extractErrorMessage } from '../../lib/errors'

describe('extractErrorMessage', () => {
  it('reads .message from a Rust ApiError-shaped object', () => {
    // Errors from Rust arrive as { code, message } plain objects, not Error
    // instances — String(e) would give "[object Object]".
    const apiError = { code: 'clipboard_error', message: 'clipboard write failed' }
    expect(extractErrorMessage(apiError, 'fallback')).toBe('clipboard write failed')
  })

  it('reads .message from a real Error instance', () => {
    expect(extractErrorMessage(new Error('boom'), 'fallback')).toBe('boom')
  })

  it('returns the fallback for values without a message', () => {
    expect(extractErrorMessage('a string', 'fallback')).toBe('fallback')
    expect(extractErrorMessage(null, 'fallback')).toBe('fallback')
    expect(extractErrorMessage(undefined, 'fallback')).toBe('fallback')
    expect(extractErrorMessage(42, 'fallback')).toBe('fallback')
  })

  it('stringifies a non-string message', () => {
    expect(extractErrorMessage({ message: 500 }, 'fallback')).toBe('500')
  })

  it('prefers an own empty message over the fallback', () => {
    // The key is present, so this takes the 'message' in e branch — showing the
    // fallback instead would misreport which error fired.
    expect(extractErrorMessage({ code: 'x', message: '' }, 'fallback')).toBe('')
  })
})
