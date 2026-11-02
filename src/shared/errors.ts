/** Extract a human-readable message from an unknown thrown value. */
export function extractErrorMessage(e: unknown, fallback: string): string {
  if (typeof e === 'object' && e !== null && 'message' in e) {
    return String((e as { message: unknown }).message)
  }
  return fallback
}

/** Whether a value thrown from Rust is the `ApiError` with this `code`. */
export function hasErrorCode(e: unknown, code: string): boolean {
  return typeof e === 'object' && e !== null && 'code' in e && e.code === code
}
