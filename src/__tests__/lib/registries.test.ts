/**
 * The COMMANDS and EVENTS registries are the contract with Rust. A name that
 * drifts fails at runtime with "command not found" and nowhere earlier — so
 * these read both sides' sources and check they still agree.
 */
import { describe, it, expect } from 'vitest'
import { COMMANDS } from '../../lib/commands'
import { EVENTS } from '../../lib/events'

// Vite inlines the matched files as text at transform time, so this needs no
// node APIs — and it picks up newly added files automatically.
const rustModules = import.meta.glob('/src-tauri/src/**/*.rs', {
  query: '?raw',
  import: 'default',
  eager: true,
}) as Record<string, string>

const tsModules = import.meta.glob('/src/**/*.{ts,tsx}', {
  query: '?raw',
  import: 'default',
  eager: true,
}) as Record<string, string>

const NEWLINE = String.fromCharCode(10)
const rustSource = Object.values(rustModules).join(NEWLINE)
const tsSource = Object.entries(tsModules)
  .filter(([path]) => !path.includes('__tests__'))
  .map(([, text]) => text)
  .join(NEWLINE)

/** Every `#[tauri::command]`-annotated fn name in the Rust sources. */
const tauriCommands = new Set(
  [...rustSource.matchAll(/#\[tauri::command\][\s\S]{0,200}?\bfn\s+(\w+)/g)].map(m => m[1]),
)

describe('COMMANDS registry', () => {
  it('found the Rust command definitions to compare against', () => {
    // Guards the test itself: a regex that matched nothing would make every
    // assertion below vacuously pass.
    expect(tauriCommands.size).toBeGreaterThan(20)
  })

  it('every registered command name exists as a #[tauri::command] in Rust', () => {
    const missing = Object.entries(COMMANDS).filter(([, name]) => !tauriCommands.has(name))
    expect(
      missing.map(([key, name]) => `${key} -> "${name}"`),
      'these invoke names have no Rust command',
    ).toEqual([])
  })

  it('has no duplicate command strings under different keys', () => {
    const names = Object.values(COMMANDS)
    expect(new Set(names).size, 'two keys share one command name').toBe(names.length)
  })

  it('names every key after its command string', () => {
    // SCREAMING_SNAKE of the snake_case value; drift makes call sites misleading.
    for (const [key, name] of Object.entries(COMMANDS)) {
      expect(key, `${key} does not match "${name}"`).toBe(name.toUpperCase())
    }
  })
})

describe('EVENTS registry', () => {
  it('every registered event is emitted by Rust or by the webview', () => {
    // Collapse whitespace so `emit( EVENTS.X` and `emit(EVENTS.X` both match.
    const compact = tsSource.replace(/\s+/g, '')

    const orphans = Object.entries(EVENTS).filter(([key, name]) => {
      const emittedByRust = rustSource.includes('"' + name + '"')
      // Webview-emitted events go out via `emit(EVENTS.KEY, …)`.
      const emittedByWebview = compact.includes('emit(EVENTS.' + key)
      return !emittedByRust && !emittedByWebview
    })

    expect(orphans.map(([k, v]) => `${k} -> "${v}"`), 'nothing emits these events').toEqual([])
  })

  it('has no duplicate event strings under different keys', () => {
    const names = Object.values(EVENTS)
    expect(new Set(names).size, 'two keys share one event name').toBe(names.length)
  })

  it('keeps every event name in the projects kebab/colon style', () => {
    // Rust and TS both hard-code these strings; a stray capital or underscore
    // is a silent mismatch.
    for (const [key, name] of Object.entries(EVENTS)) {
      expect(name, `${key} is not kebab/colon-cased`).toMatch(/^[a-z0-9]+(?:[-:][a-z0-9]+)*$/)
    }
  })
})
