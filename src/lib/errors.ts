export const ERROR_CODES = {
  TOKEN_EXPIRED: 'token_expired',
} as const

export type ErrorCode = typeof ERROR_CODES[keyof typeof ERROR_CODES]

/** Extract a human-readable message from an unknown thrown value. */
export function extractErrorMessage(e: unknown, fallback: string): string {
  if (typeof e === 'object' && e !== null && 'message' in e) {
    return String((e as { message: unknown }).message)
  }
  return fallback
}

/** Check whether a thrown value carries a specific error code from the Rust backend. */
export function isErrorCode(e: unknown, code: ErrorCode): boolean {
  return typeof e === 'object' && e !== null && 'code' in e && (e as { code: string }).code === code
}
