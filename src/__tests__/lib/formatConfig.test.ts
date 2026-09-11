/**
 * `isConfigured` greys out the Save/Test buttons. Rust's
 * `FormatConfig::is_usable` is what actually decides, so this must never be
 * stricter — a UI that refuses a config Rust would accept is a dead end.
 * The cases mirror `src-tauri/tests/unit/llm/config.rs` one for one.
 */
import { describe, it, expect } from 'vitest'
import { isConfigured } from '../../lib/formatConfig'

const profile = (baseUrl: string, model: string) => ({ baseUrl, model })

describe('isConfigured', () => {
  it('accepts a provider with both a base URL and a model', () => {
    expect(isConfigured('ollama', profile('http://localhost:11434/v1', 'qwen'))).toBe(true)
  })

  it('rejects a missing or blank model', () => {
    expect(isConfigured('ollama', profile('http://localhost:11434/v1', ''))).toBe(false)
    expect(isConfigured('ollama', profile('http://localhost:11434/v1', '   '))).toBe(false)
  })

  it('rejects a missing or blank base URL for OpenAI-compatible providers', () => {
    for (const id of ['ollama', 'lmstudio', 'openai', 'openrouter', 'custom']) {
      expect(isConfigured(id, profile('', 'a-model')), `${id} with no base URL`).toBe(false)
      expect(isConfigured(id, profile('  ', 'a-model')), `${id} blank base URL`).toBe(false)
    }
  })

  it('does not require a base URL for anthropic', () => {
    // It pins its own endpoint, so there is no field to fill — requiring one
    // would make Anthropic impossible to configure.
    expect(isConfigured('anthropic', profile('', 'claude-sonnet-5'))).toBe(true)
  })

  it('still requires a model for anthropic', () => {
    expect(isConfigured('anthropic', profile('', ''))).toBe(false)
  })

  it('never requires an API key', () => {
    // Blank means "send no auth header", which is how a local Ollama or
    // LM Studio runs. Gating on a key would lock out every local setup.
    expect(isConfigured('ollama', profile('http://localhost:11434/v1', 'qwen'))).toBe(true)
    expect(isConfigured('custom', profile('http://192.168.1.5:8080/v1', 'local'))).toBe(true)
  })

  it('treats an unknown provider as OpenAI-compatible', () => {
    // Only anthropic is special-cased; anything else needs a base URL.
    expect(isConfigured('some-new-provider', profile('', 'm'))).toBe(false)
    expect(isConfigured('some-new-provider', profile('http://x/v1', 'm'))).toBe(true)
  })

  it('ignores surrounding whitespace the way the Rust side does', () => {
    expect(isConfigured('ollama', profile('  http://x/v1  ', '  m  '))).toBe(true)
  })
})
