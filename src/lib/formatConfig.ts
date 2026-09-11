/**
 * Shared rule for whether the formatting LLM is configured enough to call.
 *
 * Lives here rather than in the component so the toggle and the provider modal
 * apply one rule instead of restating it each.
 */

/** One provider's endpoint details, as persisted in `format_config.json`. */
export type Profile = {
  baseUrl: string
  model: string
  apiKey: string
}

/**
 * Whether a provider has the minimum needed to make a request. Mirrors
 * `FormatConfig::is_usable` (llm/config.rs), which decides for real — this only
 * greys out buttons, so it must not be stricter. The API key is deliberately not
 * required: blank means no auth header, which is how local servers run.
 * Anthropic pins its own endpoint, so it needs no base URL.
 */
export function isConfigured(
  provider: string,
  profile: Pick<Profile, 'baseUrl' | 'model'>,
): boolean {
  if (profile.model.trim() === '') return false
  return provider === 'anthropic' || profile.baseUrl.trim() !== ''
}
