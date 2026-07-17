import { describe, it, expect } from 'vitest'
import { extractErrorMessage, isErrorCode, ERROR_CODES } from '../../lib/errors'

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
})

describe('isErrorCode', () => {
  it('matches a Rust ApiError with the given code', () => {
    const expired = { code: ERROR_CODES.TOKEN_EXPIRED, message: 'expired' }
    expect(isErrorCode(expired, ERROR_CODES.TOKEN_EXPIRED)).toBe(true)
  })

  it('returns false for a different or missing code', () => {
    expect(isErrorCode({ code: 'other' }, ERROR_CODES.TOKEN_EXPIRED)).toBe(false)
    expect(isErrorCode({ message: 'no code' }, ERROR_CODES.TOKEN_EXPIRED)).toBe(false)
    expect(isErrorCode(null, ERROR_CODES.TOKEN_EXPIRED)).toBe(false)
    expect(isErrorCode('string', ERROR_CODES.TOKEN_EXPIRED)).toBe(false)
  })
})
