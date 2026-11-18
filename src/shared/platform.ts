/** The host OS, from the webview's user agent. For labels only (e.g. the Super key). */

export type Platform = 'macos' | 'windows' | 'linux'

export function detectPlatform(ua = typeof navigator === 'undefined' ? '' : navigator.userAgent): Platform {
  if (/Mac|iPhone|iPad|iPod/i.test(ua)) return 'macos'
  if (/Win/i.test(ua)) return 'windows'
  return 'linux'
}

const isMac = detectPlatform() === 'macos'
export const isWindows = detectPlatform() === 'windows'

/** The label of the "Super" modifier: the Windows logo key, or Command on macOS. */
export const SUPER_KEY_LABEL = isMac ? 'Cmd' : 'Win'
