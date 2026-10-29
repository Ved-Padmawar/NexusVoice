import type { FormatterUpdate, FormatterView, ProfileView } from '../../shared/types'

/**
 * Whether a provider has the minimum needed to make a request. Mirrors
 * `FormatConfig::is_usable` (llm/config.rs), which decides for real — this only
 * greys out buttons, so it must not be stricter. The API key is deliberately not
 * required: blank means no auth header, which is how local servers run.
 * Anthropic pins its own endpoint, so it needs no base URL.
 */
export function isConfigured(
  provider: string,
  profile: Pick<ProfileView, 'baseUrl' | 'model'>,
): boolean {
  if (profile.model.trim() === '') return false
  return provider === 'anthropic' || profile.baseUrl.trim() !== ''
}

/** The saved settings as an update that keeps every saved key. */
export function keepingKeys(view: FormatterView): FormatterUpdate {
  const profiles = Object.fromEntries(
    Object.entries(view.profiles).flatMap(([id, p]) =>
      p ? [[id, { baseUrl: p.baseUrl, model: p.model, apiKey: null }]] : []),
  )
  return { enabled: view.enabled, provider: view.provider, profiles }
}
