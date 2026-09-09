import { useState, useEffect, useCallback, useRef } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Dialog } from 'radix-ui'
import { Settings2, X, CheckCircle2, AlertCircle, Plug, Save, Sparkles } from 'lucide-react'
import { COMMANDS } from '../lib/commands'
import { extractErrorMessage } from '../lib/errors'
import { VendorMark } from './ui/VendorMark'
import { Section } from './Section'
import { Switch } from './Switch'
import { Spinner } from './Spinner'
import type { VendorId } from '../lib/vendors'
import { toast } from 'sonner'

type Profile = {
  baseUrl: string
  model: string
  apiKey: string
}

type FormatConfig = {
  enabled: boolean
  provider: string
  profiles: Record<string, Profile>
}

const EMPTY_PROFILE: Profile = { baseUrl: '', model: '', apiKey: '' }

const activeProfile = (c: FormatConfig): Profile =>
  c.profiles?.[c.provider] ?? EMPTY_PROFILE

type Preset = {
  id: string
  label: string
  baseUrl: string
  needsKey: boolean
  modelHint: string
}

// Every preset but "anthropic" speaks the OpenAI-compatible API; presets only
// prefill the base URL, and "anthropic"/"custom" prefill nothing.
const PRESETS: Preset[] = [
  { id: 'ollama',     label: 'Ollama', baseUrl: 'http://localhost:11434/v1', needsKey: false, modelHint: 'qwen2.5:3b-instruct' },
  { id: 'lmstudio',   label: 'LM Studio', baseUrl: 'http://localhost:1234/v1',  needsKey: false, modelHint: 'qwen2.5-3b-instruct' },
  { id: 'openai',     label: 'OpenAI',            baseUrl: 'https://api.openai.com/v1',  needsKey: true,  modelHint: 'gpt-5.5' },
  { id: 'openrouter', label: 'OpenRouter',        baseUrl: 'https://openrouter.ai/api/v1', needsKey: true, modelHint: 'meta-llama/llama-3.1-8b-instruct' },
  { id: 'anthropic',  label: 'Anthropic',         baseUrl: '',                            needsKey: true,  modelHint: 'claude-sonnet-5' },
  { id: 'custom',     label: 'Custom',            baseUrl: '',                            needsKey: false, modelHint: 'model name' },
]

const DEFAULT_CONFIG: FormatConfig = { enabled: false, provider: 'ollama', profiles: {} }

/** Smart-formatting toggle + provider config. The formatter is an HTTP call,
 *  off by default; enabling without an endpoint opens the modal. */
export function FormattingToggle() {
  const [config, setConfig] = useState<FormatConfig>(DEFAULT_CONFIG)
  const [modalOpen, setModalOpen] = useState(false)

  const refresh = useCallback(() => {
    invoke<Partial<FormatConfig> | null>(COMMANDS.GET_FORMAT_CONFIG)
      // A missing config must not blank out the defaults.
      .then((c) => setConfig({ ...DEFAULT_CONFIG, ...(c ?? {}) }))
      .catch(() => {})
  }, [])

  useEffect(() => { refresh() }, [refresh])

  const active = activeProfile(config)
  const configured =
    active.model.trim() !== '' &&
    (config.provider === 'anthropic' || active.baseUrl.trim() !== '')

  const persist = async (next: FormatConfig) => {
    setConfig(next)
    await invoke(COMMANDS.SET_FORMAT_CONFIG, { config: next }).catch(() => {
      toast.error('Failed to save formatting settings')
    })
  }

  const toggle = async () => {
    const next = !config.enabled
    if (next && !configured) {
      // On while the endpoint is set up; closing unsaved puts it back.
      setConfig({ ...config, enabled: true })
      setModalOpen(true)
      return
    }
    await persist({ ...config, enabled: next })
  }

  const presetLabel = PRESETS.find((p) => p.id === config.provider)?.label ?? config.provider

  return (
    <>
      <Section
        title="Smart formatting"
        Icon={Sparkles}
        description="Cleans up your dictation: punctuates it properly and turns spoken lists into actual lists."
        action={
          <div className="flex items-center gap-2.5">
            <button type="button" onClick={() => setModalOpen(true)} className="btn btn-sm btn-quiet">
              <Settings2 size={12} strokeWidth={1.9} />
              Configure
            </button>
            <Switch checked={config.enabled} onChange={toggle} label="Toggle smart formatting" />
          </div>
        }
        bodyClassName="px-4 py-3"
      >
        <div className="flex items-center gap-2 text-[11.5px]">
          {configured ? (
            <>
              <VendorMark vendor={config.provider as VendorId} className="size-3.5 shrink-0" />
              <span className="text-(--fg-2)">{presetLabel}</span>
              <span className="rule h-2.5 w-px" />
              <span className="min-w-0 truncate font-medium text-(--fg)">{active.model}</span>
            </>
          ) : (
            <span className="text-(--muted)">No endpoint configured yet</span>
          )}
        </div>
      </Section>

      {modalOpen && (
        <ProviderModal
          initial={config}
          onClose={() => {
            setModalOpen(false)
            if (!configured) setConfig((c) => ({ ...c, enabled: false }))
          }}
          onSaved={(saved) => { setConfig(saved); setModalOpen(false) }}
        />
      )}
    </>
  )
}

function ProviderModal({
  initial,
  onClose,
  onSaved,
}: {
  initial: FormatConfig
  onClose: () => void
  onSaved: (c: FormatConfig) => void
}) {
  const [providerId, setProviderId] = useState(initial.provider || 'ollama')
  const seed = activeProfile(initial)
  const [baseUrl, setBaseUrl] = useState(seed.baseUrl)
  const [model, setModel] = useState(seed.model)
  const [apiKey, setApiKey] = useState(seed.apiKey)
  const [testing, setTesting] = useState(false)
  const [testResult, setTestResult] = useState<'idle' | 'ok' | 'fail'>('idle')
  const [saving, setSaving] = useState(false)
  const resultTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const preset = PRESETS.find((p) => p.id === providerId) ?? PRESETS[0]
  // Anthropic pins its own endpoint (native Messages API) — a base URL field
  // would be misleading, so it's hidden entirely for that provider.
  const showBaseUrl = providerId !== 'anthropic'

  // Clean up the result-flash timer on unmount.
  useEffect(() => () => { if (resultTimerRef.current) clearTimeout(resultTimerRef.current) }, [])

  const selectProvider = (id: string) => {
    setProviderId(id)
    setTestResult('idle')
    // Preset URL is only a starting point for a provider never configured.
    const saved = initial.profiles?.[id]
    const preset = PRESETS.find((x) => x.id === id)
    const fallbackUrl = preset && id !== 'custom' && id !== 'anthropic' ? preset.baseUrl : ''
    setBaseUrl(saved?.baseUrl || fallbackUrl)
    setModel(saved?.model ?? '')
    setApiKey(saved?.apiKey ?? '')
  }

  const draft = (): FormatConfig => ({
    enabled: initial.enabled,
    provider: providerId,
    profiles: {
      ...initial.profiles,
      [providerId]: { baseUrl: baseUrl.trim(), model: model.trim(), apiKey: apiKey.trim() },
    },
  })

  // Mirrors `FormatConfig::is_usable` (llm/config.rs), which decides for real —
  // this only greys out the buttons. The key is optional there (blank means no
  // auth header, for local servers), so it must not gate submission here.
  const canSubmit =
    model.trim() !== '' &&
    (!showBaseUrl || baseUrl.trim() !== '')

  // Show the test result icon briefly, then revert the button to its ready
  // state — leaving it stuck on a past result is misleading.
  const flashResult = (result: 'ok' | 'fail') => {
    setTestResult(result)
    if (resultTimerRef.current) clearTimeout(resultTimerRef.current)
    resultTimerRef.current = setTimeout(() => setTestResult('idle'), 4000)
  }

  const handleTest = async () => {
    if (!canSubmit) return
    setTesting(true)
    setTestResult('idle')
    try {
      await invoke(COMMANDS.TEST_FORMAT_CONNECTION, { config: draft() })
      flashResult('ok')
      toast.success('Connection successful')
    } catch (e) {
      flashResult('fail')
      const msg = extractErrorMessage(e, 'could not reach endpoint')
      toast.error(`Connection failed: ${msg}`)
    } finally {
      setTesting(false)
    }
  }

  const handleSave = async () => {
    if (!canSubmit) return
    setSaving(true)
    // Saving from the modal also enables formatting — the user configured an
    // endpoint specifically to use it.
    const next = { ...draft(), enabled: true }
    try {
      await invoke(COMMANDS.SET_FORMAT_CONFIG, { config: next })
      toast.success('Smart formatting enabled')
      onSaved(next)
    } catch {
      toast.error('Failed to save settings')
    } finally {
      setSaving(false)
    }
  }

  return (
    <Dialog.Root open onOpenChange={(next) => { if (!next) onClose() }}>
      <Dialog.Portal>
        <Dialog.Overlay className="scrim fixed inset-0 z-50 data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=closed]:animate-out data-[state=closed]:fade-out-0" />
        <Dialog.Content
          aria-describedby={undefined}
          onOpenAutoFocus={(e) => e.preventDefault()}
          className="pop fixed left-1/2 top-1/2 z-50 flex w-115 -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden
                     data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95
                     data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95"
        >
          <div className="flex items-center justify-between gap-4 px-5 py-4">
            <div className="min-w-0">
              <Dialog.Title className="m-0 text-[14px] font-semibold tracking-[-0.01em] text-(--fg)">
                Formatting provider
              </Dialog.Title>
              <p className="m-0 mt-0.5 text-[11.5px] text-(--muted)">
                Connect Anthropic, or any OpenAI-compatible endpoint.
              </p>
            </div>
            <Dialog.Close asChild>
              <button type="button" aria-label="Close" className="iconbtn iconbtn-danger">
                <X size={14} strokeWidth={2} />
              </button>
            </Dialog.Close>
          </div>
          <div className="rule h-px" />

          <div className="flex flex-col gap-4 px-5 py-4">
            <div className="grid grid-cols-3 gap-1.5">
              {PRESETS.map((p) => (
                <button
                  key={p.id}
                  type="button"
                  onClick={() => selectProvider(p.id)}
                  aria-pressed={p.id === providerId}
                  className="pick flex items-center gap-2 px-2.5 py-2 text-[11.5px] font-medium"
                >
                  <VendorMark vendor={p.id as VendorId} className="size-4 shrink-0" />
                  <span className={`min-w-0 truncate ${p.id === providerId ? 'text-(--on-soft)' : 'text-(--fg-2)'}`}>
                    {p.label}
                  </span>
                </button>
              ))}
            </div>

            {showBaseUrl && (
              <Field label="Base URL">
                <input
                  id="format-base-url"
                  name="format-base-url"
                  type="text"
                  autoComplete="off"
                  value={baseUrl}
                  onChange={(e) => { setBaseUrl(e.target.value); setTestResult('idle') }}
                  placeholder={preset.baseUrl || 'http://localhost:11434/v1'}
                  className="field"
                  spellCheck={false}
                />
              </Field>
            )}

            <Field
              label="Model"
              hint="Use a small instruct model, not a reasoning one — it is faster and reformats rather than replies."
            >
              <input
                id="format-model"
                name="format-model"
                type="text"
                autoComplete="off"
                value={model}
                onChange={(e) => { setModel(e.target.value); setTestResult('idle') }}
                placeholder={preset.modelHint}
                className="field"
                spellCheck={false}
              />
            </Field>

            <Field label={preset.needsKey ? 'API key' : 'API key (optional)'}>
              <input
                id="format-api-key"
                name="format-api-key"
                type="password"
                autoComplete="off"
                value={apiKey}
                onChange={(e) => { setApiKey(e.target.value); setTestResult('idle') }}
                placeholder={preset.needsKey ? 'sk-…' : 'Leave blank for local servers'}
                className="field"
                spellCheck={false}
              />
            </Field>
          </div>

          <div className="rule h-px" />
          <div className="flex items-center justify-between gap-3 px-5 py-4">
            <button
              type="button"
              onClick={handleTest}
              disabled={!canSubmit || testing}
              className="btn btn-quiet"
            >
              {testing ? <Spinner size={13} />
                : testResult === 'ok' ? <CheckCircle2 size={13} className="text-(--success)" />
                : testResult === 'fail' ? <AlertCircle size={13} className="text-(--danger)" />
                : <Plug size={13} strokeWidth={1.9} />}
              Test connection
            </button>
            <button
              type="button"
              onClick={handleSave}
              disabled={!canSubmit || saving}
              className="btn btn-primary px-4"
            >
              {saving ? <Spinner size={13} /> : <Save size={13} strokeWidth={1.9} />}
              {saving ? 'Saving…' : 'Save and enable'}
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  )
}

function Field({ label, hint, children }: { label: string; hint?: string; children: React.ReactNode }) {
  return (
    <label className="flex flex-col gap-1.5">
      <span className="text-[11px] text-(--muted)">{label}</span>
      {children}
      {hint && <span className="text-[10.5px] leading-[1.5] text-(--muted)">{hint}</span>}
    </label>
  )
}
