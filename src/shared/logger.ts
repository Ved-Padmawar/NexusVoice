import { commands } from './bindings'

type LogLevel = 'error' | 'warn' | 'info' | 'debug'

/** Sends UI logs to Rust's log file. Use instead of `console.*`; a failed
 *  forward is dropped, never thrown. */
function forward(level: LogLevel, message: string, context?: unknown): void {
  let ctx: string | undefined
  if (context !== undefined) {
    try {
      ctx = typeof context === 'string' ? context : JSON.stringify(context)
    } catch {
      ctx = String(context)
    }
  }
  commands.logFrontend(level, message, ctx ?? null).catch(() => {})
}

export const logger = {
  error: (message: string, context?: unknown) => forward('error', message, context),
  warn: (message: string, context?: unknown) => forward('warn', message, context),
  info: (message: string, context?: unknown) => forward('info', message, context),
  debug: (message: string, context?: unknown) => forward('debug', message, context),
}
