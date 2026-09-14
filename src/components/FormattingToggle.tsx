import { useState, useEffect, useCallback, useRef, type ChangeEvent, type ReactNode } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { AnimatePresence } from 'framer-motion'
import { Sparkles, Settings2, CheckCircle2, AlertCircle, Plug, Loader2 } from 'lucide-react'
import { toast } from 'sonner'
import { COMMANDS } from '../lib/commands'
import { extractErrorMessage } from '../lib/errors'
import { isConfigured, type Profile } from '../lib/formatConfig'
import type { VendorId } from '../lib/vendors'
import { VendorMark } from './ui/VendorMark'
import { Button } from './ui/button'
import { Input } from './ui/input'
import { Switch } from './ui/switch'
import { Modal, ModalBody, ModalFoot } from './ui/modal'
import { SettingRow } from './page'

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
  { id: 'ollama',     label: 'Ollama',     baseUrl: 'http://localhost:11434/v1',    needsKey: false, modelHint: 'qwen2.5:3b-instruct' },
  { id: 'lmstudio',   label: 'LM Studio',  baseUrl: 'http://localhost:1234/v1',     needsKey: false, modelHint: 'qwen2.5-3b-instruct' },
  { id: 'openai',     label: 'OpenAI',     baseUrl: 'https://api.openai.com/v1',    needsKey: true,  modelHint: 'gpt-5.5' },
  { id: 'openrouter', label: 'OpenRouter', baseUrl: 'https://openrouter.ai/api/v1', needsKey: true,  modelHint: 'meta-llama/llama-3.1-8b-instruct' },
  { id: 'anthropic',  label: 'Anthropic',  baseUrl: '',                             needsKey: true,  modelHint: 'claude-sonnet-5' },
  { id: 'custom',     label: 'Custom',     baseUrl: '',                             needsKey: false, modelHint: 'model name' },
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
  const configured = isConfigured(config.provider, active)

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
      <SettingRow
        title={<>Smart formatting <span className="nv-badge nv-badge--neutral">Optional</span></>}
        description={
          <>
            <span className="mb-1.5 flex items-center gap-1.5 font-medium text-fg-2">
              {configured ? (
                <>
                  <VendorMark vendor={config.provider as VendorId} className="size-3.5 shrink-0" />
                  <span className="min-w-0 truncate">{presetLabel}, {active.model}</span>
                </>
              ) : 'No endpoint configured'}
            </span>
            Sends each transcript to a language model you choose to fix punctuation and turn
            spoken lists into real ones before it is pasted.
          </>
        }
        inline
      >
        <Button variant="secondary" size="sm" onClick={() => setModalOpen(true)}>
          <Settings2 />
          Configure
        </Button>
        <Switch checked={config.enabled} onChange={() => void toggle()} label="Toggle smart formatting" />
      </SettingRow>

      <AnimatePresence>
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
      </AnimatePresence>
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

  const canSubmit = isConfigured(providerId, { baseUrl, model })

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
      toast.error(`Connection failed: ${extractErrorMessage(e, 'could not reach endpoint')}`)
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
      toast.success('Smart formatting turned on')
      onSaved(next)
    } catch {
      toast.error('Failed to save settings')
    } finally {
      setSaving(false)
    }
  }

  const edit = (set: (v: string) => void) => (e: ChangeEvent<HTMLInputElement>) => {
    set(e.target.value)
    setTestResult('idle')
  }

  return (
    <Modal
      onClose={onClose}
      title="Formatting provider"
      description="Anthropic, or any server that speaks the OpenAI API."
      icon={<span className="nv-mark nv-mark--accent"><Sparkles size={16} strokeWidth={2} /></span>}
      width={500}
    >
      <ModalBody className="flex flex-col gap-4">
        <div role="radiogroup" aria-label="Provider" className="grid grid-cols-2 gap-2 sm:grid-cols-3">
          {PRESETS.map((p) => (
            <button
              key={p.id}
              type="button"
              role="radio"
              aria-checked={p.id === providerId}
              onClick={() => selectProvider(p.id)}
              className="nv-option items-center! py-2.5!"
            >
              <VendorMark vendor={p.id as VendorId} className="size-4 shrink-0" />
              <span className="min-w-0 truncate text-[12.5px] font-semibold">{p.label}</span>
            </button>
          ))}
        </div>

        {showBaseUrl && (
          <Field label="Base URL">
            <Input
              id="format-base-url"
              name="format-base-url"
              autoComplete="off"
              spellCheck={false}
              value={baseUrl}
              onChange={edit(setBaseUrl)}
              placeholder={preset.baseUrl || 'http://localhost:11434/v1'}
            />
          </Field>
        )}

        <Field
          label="Model"
          hint="Use a small instruct model such as qwen2.5-3b-instruct, not a reasoning model. It is faster and sticks to your words."
        >
          <Input
            id="format-model"
            name="format-model"
            autoComplete="off"
            spellCheck={false}
            value={model}
            onChange={edit(setModel)}
            placeholder={preset.modelHint}
          />
        </Field>

        <Field label={preset.needsKey ? 'API key' : 'API key (optional)'}>
          <Input
            id="format-api-key"
            name="format-api-key"
            type="password"
            autoComplete="off"
            spellCheck={false}
            value={apiKey}
            onChange={edit(setApiKey)}
            placeholder={preset.needsKey ? 'sk-…' : 'Leave blank for local servers'}
          />
        </Field>
      </ModalBody>

      <ModalFoot>
        <Button variant="secondary" onClick={handleTest} disabled={!canSubmit || testing}>
          {testing ? <Loader2 className="nv-spin" />
            : testResult === 'ok' ? <CheckCircle2 className="text-success" />
            : testResult === 'fail' ? <AlertCircle className="text-danger" />
            : <Plug />}
          Test connection
        </Button>
        <Button onClick={handleSave} disabled={!canSubmit || saving}>
          {saving && <Loader2 className="nv-spin" />}
          {saving ? 'Saving…' : 'Save and turn on'}
        </Button>
      </ModalFoot>
    </Modal>
  )
}

function Field({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <label className="flex flex-col">
      <span className="nv-field-label">{label}</span>
      {children}
      {hint && <span className="nv-hint mt-1.5">{hint}</span>}
    </label>
  )
}
