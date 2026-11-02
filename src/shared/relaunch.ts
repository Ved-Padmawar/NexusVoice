import { toast } from 'sonner'
import { commands } from './bindings'
import { extractErrorMessage } from './errors'

/** Restart the app. Rust refuses while a dictation is still being recorded or transcribed. */
export async function relaunchApp() {
  try {
    await commands.relaunchApp()
  } catch (e) {
    toast.error(extractErrorMessage(e, 'Could not restart'))
  }
}
