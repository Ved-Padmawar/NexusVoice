import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useShallow } from 'zustand/react/shallow'
import { COMMANDS } from './commands'
import { SUPER_KEY_LABEL } from './platform'
import { useAppStore } from '../store/useAppStore'
import { parseRegisteredHotkeys } from '../store/modelSlice'

export type HotkeyKind = 'ptt' | 'dictation' | 'dictationCommit'

export type RegisteredHotkeys = Record<HotkeyKind, string | null>

const NONE: RegisteredHotkeys = { ptt: null, dictation: null, dictationCommit: null }

export function getKeyName(key: string, code: string): string {
  const map: Record<string, string> = {
    Control: 'Ctrl', Meta: 'Super', ' ': 'Space',
    ArrowUp: 'ArrowUp', ArrowDown: 'ArrowDown',
    ArrowLeft: 'ArrowLeft', ArrowRight: 'ArrowRight',
    Escape: 'Escape', Delete: 'Delete',
    Backspace: 'Backspace', Enter: 'Return', Tab: 'Tab',
  }
  if (map[key]) return map[key]
  if (key.length === 1) return key.toUpperCase()
  if (/^F\d+$/.test(key)) return key
  if (code.startsWith('Key')) return code.slice(3)
  if (code.startsWith('Digit')) return code.slice(5)
  return key
}

const KEY_DISPLAY: Record<string, string> = {
  // Super is the Windows logo key on Win/Linux and Command on macOS — the
  // internal accelerator token stays "Super" everywhere; only the label differs.
  Ctrl: 'Ctrl', Super: SUPER_KEY_LABEL, Return: 'Enter',
  Backspace: 'Backspace', Delete: 'Del', Escape: 'Esc',
  ArrowUp: 'Up', ArrowDown: 'Down', ArrowLeft: 'Left', ArrowRight: 'Right',
}

export const displayKey = (k: string) => KEY_DISPLAY[k] ?? k

export function buildShortcut(keys: string[]): string {
  const ORDER = ['Ctrl', 'Alt', 'Shift', 'Super']
  const mods: string[] = []
  let main = ''
  for (const k of keys) {
    if (['Ctrl', 'Alt', 'Shift', 'Win', 'Cmd'].includes(k)) {
      mods.push(k === 'Win' || k === 'Cmd' ? 'Super' : k)
    } else {
      main = k
    }
  }
  mods.sort((a, b) => ORDER.indexOf(a) - ORDER.indexOf(b))
  return main ? [...mods, main].join('+') : mods.join('+')
}

/** The store only holds presence flags; the accelerators are fetched. */
export function useRegisteredHotkeys() {
  const { hasHotkey, hasDictationHotkey, hasDictationCommitHotkey } = useAppStore(useShallow(s => ({
    hasHotkey: s.hasHotkey,
    hasDictationHotkey: s.hasDictationHotkey,
    hasDictationCommitHotkey: s.hasDictationCommitHotkey,
  })))
  const [hotkeys, setHotkeys] = useState<RegisteredHotkeys>(NONE)

  useEffect(() => {
    if (!hasHotkey && !hasDictationHotkey && !hasDictationCommitHotkey) return
    let cancelled = false
    invoke<unknown>(COMMANDS.GET_REGISTERED_HOTKEYS)
      .then(raw => {
        if (cancelled) return
        const parsed = parseRegisteredHotkeys(raw)
        setHotkeys({
          ptt: parsed.ptt[0] ?? null,
          dictation: parsed.dictation[0] ?? null,
          dictationCommit: parsed.dictationCommit[0] ?? null,
        })
      })
      .catch(() => {})
    return () => { cancelled = true }
  }, [hasHotkey, hasDictationHotkey, hasDictationCommitHotkey])

  return [hotkeys, setHotkeys] as const
}
