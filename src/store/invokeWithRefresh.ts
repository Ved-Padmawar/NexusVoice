import { invoke } from '@tauri-apps/api/core'
import { COMMANDS } from '../lib/commands'
import { ERROR_CODES, isErrorCode } from '../lib/errors'
import { TokenPairResponseSchema } from '../types'

// Forward-declared to avoid circular import — set by useAppStore after creation
let storeRef: {
  getState: () => { refreshToken: string | null; user: { id: number } | null; logout: () => Promise<void> }
  setState: (s: { refreshToken: string }) => void
} | null = null

export function setStoreRef(ref: typeof storeRef) {
  storeRef = ref
}

/** Wraps invoke — if token_expired, refreshes once then retries; logs out if refresh also fails. */
export async function invokeWithRefresh<T>(
  cmd: string,
  args?: Record<string, unknown>
): Promise<T> {
  try {
    return await invoke<T>(cmd, args)
  } catch (e) {
    if (!isErrorCode(e, ERROR_CODES.TOKEN_EXPIRED)) throw e
    const store = storeRef?.getState()
    const stored = store?.refreshToken
    if (!stored) {
      await storeRef?.getState().logout()
      throw e
    }
    try {
      const raw = await invoke<unknown>(COMMANDS.REFRESH_TOKEN, { refreshToken: stored })
      const pair = TokenPairResponseSchema.parse(raw)
      await invoke(COMMANDS.STORE_REFRESH_TOKEN, {
        refreshToken: pair.refreshToken,
        userId: storeRef?.getState().user?.id,
        accessToken: pair.accessToken,
      })
      storeRef?.setState({ refreshToken: pair.refreshToken })
      return await invoke<T>(cmd, args)
    } catch {
      await storeRef?.getState().logout()
      throw e
    }
  }
}
