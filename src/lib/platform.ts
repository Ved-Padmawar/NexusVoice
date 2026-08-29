/**
 * Lightweight host-OS detection for the webview. Used only for cosmetic,
 * OS-appropriate labelling (e.g. the Super/Command key). Reads the webview's
 * user-agent — no Tauri OS plugin needed for a display-only concern.
 */

export type Platform = 'macos' | 'windows' | 'linux'

function detectPlatform(): Platform {
  const ua = typeof navigator === 'undefined' ? '' : navigator.userAgent
  if (/Mac|iPhone|iPad|iPod/i.test(ua)) return 'macos'
  if (/Win/i.test(ua)) return 'windows'
  return 'linux'
}

export const isMac = detectPlatform() === 'macos'

/**
 * Display label for the "Super" modifier (Windows logo / Command key). The
 * internal accelerator token stays `Super` on every OS — this is label-only.
 */
export const SUPER_KEY_LABEL = isMac ? 'Cmd' : 'Win'
