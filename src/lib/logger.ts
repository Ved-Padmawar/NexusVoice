import { invoke } from '@tauri-apps/api/core'
import { COMMANDS } from './commands'

type LogLevel = 'error' | 'warn' | 'info' | 'debug'

/**
 * Frontend logger. Forwards records to the Rust backend so UI logs land in the
 * same unified, structured log file as backend events. Use instead of `console.*`.
 * Best-effort: a failed forward is swallowed (never throws into UI flows).
 */
function forward(level: LogLevel, message: string, context?: unknown): void {
  let ctx: string | undefined
  if (context !== undefined) {
    try {
      ctx = typeof context === 'string' ? context : JSON.stringify(context)
    } catch {
      ctx = String(context)
    }
  }
  invoke(COMMANDS.LOG_FRONTEND, { level, message, context: ctx ?? null }).catch(() => {})
}

export const logger = {
  error: (message: string, context?: unknown) => forward('error', message, context),
  warn: (message: string, context?: unknown) => forward('warn', message, context),
  info: (message: string, context?: unknown) => forward('info', message, context),
  debug: (message: string, context?: unknown) => forward('debug', message, context),
}
