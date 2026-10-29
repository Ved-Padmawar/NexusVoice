import { useState, useEffect, useCallback, useRef } from 'react'
import {
  SparkleIcon, SlidersHorizontalIcon, XIcon, CheckIcon, CheckCircleIcon, SpinnerGapIcon, WarningCircleIcon,
  PlugIcon, PlugsConnectedIcon, LinkIcon, CubeIcon, KeyIcon, LockSimpleIcon,
} from '@phosphor-icons/react'
import { commands } from '../../shared/bindings'
import { extractErrorMessage } from '../../shared/errors'
import { VendorMark } from '../../shared/ui/VendorMark'
import { Button } from '../../shared/ui/Button'
import { Modal } from '../../shared/ui/Modal'
import { Switch } from '../../shared/ui/Switch'
import { TextInput } from '../../shared/ui/TextInput'
import type { VendorId } from '../../shared/vendors'
import { toast } from 'sonner'
import { isConfigured, keepingKeys } from './formatConfig'
import type { FormatterUpdate, FormatterView, ProfileView } from '../../shared/types'

const EMPTY_PROFILE: ProfileView = { baseUrl: '', model: '', hasApiKey: false }

const activeProfile = (c: FormatterView): ProfileView =>
  c.profiles[c.provider] ?? EMPTY_PROFILE

type Preset = {
  id: string
  label: string
  hint: string
  baseUrl: string
  needsKey: boolean
  modelHint: string
}

// Every preset but "anthropic" speaks the OpenAI-compatible API; presets only
// prefill the base URL, and "anthropic"/"custom" prefill nothing.
const PRESETS: Preset[] = [
  { id: 'ollama',     label: 'Ollama',     hint: 'Local or cloud', baseUrl: 'http://localhost:11434/v1',    needsKey: false, modelHint: 'qwen2.5:3b-instruct' },
  { id: 'lmstudio',   label: 'LM Studio',  hint: 'Local',          baseUrl: 'http://localhost:1234/v1',     needsKey: false, modelHint: 'qwen2.5-3b-instruct' },
  { id: 'openai',     label: 'OpenAI',     hint: 'API key',        baseUrl: 'https://api.openai.com/v1',    needsKey: true,  modelHint: 'gpt-5.5' },
  { id: 'openrouter', label: 'OpenRouter', hint: 'API key',        baseUrl: 'https://openrouter.ai/api/v1', needsKey: true,  modelHint: 'meta-llama/llama-3.1-8b-instruct' },
  { id: 'anthropic',  label: 'Anthropic',  hint: 'API key',        baseUrl: '',                             needsKey: true,  modelHint: 'claude-haiku-5-5' },
  { id: 'custom',     label: 'Custom',     hint: 'Any OpenAI-compatible', baseUrl: '',                      needsKey: false, modelHint: 'your-model-name' },
]

const DEFAULT_CONFIG: FormatterView = { enabled: false, provider: 'ollama', profiles: {} }

/** Host and path of a base URL, without the scheme. */
const hostOf = (url: string) => url.replace(/^https?:\/\//, '').replace(/\/+$/, '')

/** Smart-formatting toggle + provider config. The formatter is an HTTP call,
 *  off by default; enabling without an endpoint opens the modal. */
export function FormattingToggle() {
  const [config, setConfig] = useState<FormatterView>(DEFAULT_CONFIG)
  const [modalOpen, setModalOpen] = useState(false)
  // Bumped per opening, so the form starts from the saved config each time.
  const [formKey, setFormKey] = useState(0)
  const openModal = () => {
    setFormKey((k) => k + 1)
    setModalOpen(true)
  }

  const refresh = useCallback(() => {
    commands.getFormatConfig().then(setConfig).catch(() => {})
  }, [])

  useEffect(() => { refresh() }, [refresh])

  const active = activeProfile(config)
  const configured = isConfigured(config.provider, active)
  const dim = config.enabled ? '' : 'opacity-50'

  const persist = async (next: FormatterView) => {
    setConfig(next)
    await commands.setFormatConfig(keepingKeys(next)).catch(() => {
      toast.error('Failed to save formatting settings')
    })
  }

  const toggle = async (next: boolean) => {
    if (next && !configured) {
      // On while the endpoint is set up; closing unsaved puts it back.
      setConfig({ ...config, enabled: true })
      openModal()
      return
    }
    await persist({ ...config, enabled: next })
  }

  const close = () => {
    setModalOpen(false)
    if (!configured) setConfig((c) => ({ ...c, enabled: false }))
  }

  const preset = PRESETS.find((p) => p.id === config.provider)

  return (
    <div className="flex flex-col gap-3 rounded-2xl bg-panel p-4 ring-1 ring-line-soft ring-inset">
      <div className="flex items-center gap-3">
        <span className="grid size-8 place-items-center rounded-[10px] bg-accent-soft text-base text-accent-ink">
          <SparkleIcon size={16} weight="duotone" />
        </span>
        <p className="flex-1 text-sm font-semibold text-fg">Smart formatting</p>
        <Switch checked={config.enabled} onChange={(on) => void toggle(on)} aria-label="Smart formatting" />
      </div>
      <p className="text-xs text-pretty text-muted">
        Sends each transcript to your LLM to fix punctuation and turn spoken lists into real ones. If it fails, you get the raw transcript.
      </p>

      {/* The formatter in use, at the same height as the live-transcript
          preview beside it, so the two cards line up. */}
      <div className={`mt-auto flex h-20 items-center gap-3 rounded-lg bg-field px-4 ring-1 ring-line-soft ring-inset`}>
        <span className={`grid size-11 shrink-0 place-items-center rounded-xl bg-panel shadow-(--shadow-sm) ring-1 ring-line-soft ring-inset transition-opacity ${dim}`}>
          {configured
            ? <VendorMark vendor={config.provider as VendorId} className="size-5.5" />
            : <PlugIcon size={20} className="text-muted" />}
        </span>
        <span className={`min-w-0 flex-1 transition-opacity ${dim}`}>
          {configured ? (
            <>
              <span className="block text-sm font-semibold text-fg">{preset?.label ?? config.provider}</span>
              <span className="block truncate text-xs text-muted">
                <span className="font-mono text-fg-2">{active.model}</span>
                {active.baseUrl && ` at ${hostOf(active.baseUrl)}`}
              </span>
            </>
          ) : (
            <span className="block text-sm font-semibold text-fg">No endpoint configured</span>
          )}
        </span>
        <Button variant="chip" onClick={openModal} className="hover:text-accent-ink hover:ring-accent-line">
          <SlidersHorizontalIcon size={12} weight="bold" />
          Configure
        </Button>
      </div>

      <Modal open={modalOpen} aria-labelledby="format-title" className="w-135" onClose={close}>
        <ProviderForm key={formKey} initial={config} onClose={close} onSaved={() => { refresh(); setModalOpen(false) }} />
      </Modal>
    </div>
  )
}

function ProviderForm({
  initial,
  onClose,
  onSaved,
}: {
  initial: FormatterView
  onClose: () => void
  onSaved: () => void
}) {
  const [providerId, setProviderId] = useState(initial.provider || 'ollama')
  const seed = activeProfile(initial)
  const [baseUrl, setBaseUrl] = useState(seed.baseUrl)
  const [model, setModel] = useState(seed.model)
  // The saved key never comes back from the backend: typing replaces it.
  const [apiKey, setApiKey] = useState('')
  const [keySaved, setKeySaved] = useState(seed.hasApiKey)
  const [removeKey, setRemoveKey] = useState(false)
  const [testing, setTesting] = useState(false)
  const [testResult, setTestResult] = useState<'idle' | 'ok' | 'fail'>('idle')
  const [saving, setSaving] = useState(false)
  const resultTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const preset = PRESETS.find((p) => p.id === providerId) ?? PRESETS[0]
  // Anthropic pins its own endpoint, so a base URL field would mislead.
  const showBaseUrl = providerId !== 'anthropic'

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
    setApiKey('')
    setKeySaved(saved?.hasApiKey ?? false)
    setRemoveKey(false)
  }

  // Only the edited provider can change its key; the rest keep theirs.
  const draft = (): FormatterUpdate => {
    const typed = apiKey.trim()
    const key = typed !== '' ? typed : removeKey ? '' : null
    const base = keepingKeys(initial)
    return {
      ...base,
      provider: providerId,
      profiles: { ...base.profiles, [providerId]: { baseUrl: baseUrl.trim(), model: model.trim(), apiKey: key } },
    }
  }

  const canSubmit = isConfigured(providerId, { baseUrl, model })

  // The test result shows briefly; a stale one would mislead.
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
      await commands.testFormatConnection(draft())
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
    // Saving also enables formatting: an endpoint is configured to be used.
    try {
      await commands.setFormatConfig({ ...draft(), enabled: true })
      toast.success('Smart formatting enabled')
      onSaved()
    } catch (e) {
      toast.error(extractErrorMessage(e, 'Failed to save settings'))
    } finally {
      setSaving(false)
    }
  }

  return (
    <>
      <div className="flex items-center gap-3 px-6 pt-5 pb-4">
        <div className="flex-1">
          <h2 id="format-title" className="text-xl font-bold tracking-tight text-fg">Smart formatting</h2>
          <p className="text-xs text-muted">Connect Anthropic or any OpenAI-compatible endpoint.</p>
        </div>
        <button
          type="button"
          aria-label="Close"
          onClick={onClose}
          className="grid size-8 cursor-pointer place-items-center rounded-lg text-muted hover:bg-danger-soft hover:text-danger"
        >
          <XIcon size={14} weight="bold" />
        </button>
      </div>

      <div className="flex min-h-0 flex-col gap-4 overflow-y-auto px-6 pb-5">
        <div role="radiogroup" aria-label="Provider" className="grid grid-cols-3 gap-1.5">
          {PRESETS.map((p) => {
            const on = p.id === providerId
            return (
              <button
                key={p.id}
                type="button"
                role="radio"
                aria-checked={on}
                onClick={() => selectProvider(p.id)}
                className={`flex cursor-pointer items-center gap-2.5 rounded-xl px-2.5 py-2 text-left transition ${
                  on ? 'bg-accent-soft ring-2 ring-accent ring-inset' : 'bg-field ring-1 ring-transparent ring-inset hover:ring-line'
                }`}
              >
                <span className="grid size-8 shrink-0 place-items-center rounded-lg bg-panel ring-1 ring-line-soft ring-inset">
                  {p.id === 'custom'
                    ? <PlugsConnectedIcon size={16} weight="bold" className="text-fg-2" />
                    : <VendorMark vendor={p.id as VendorId} className="size-4.5" />}
                </span>
                <span className="min-w-0">
                  <span className={`block text-sm font-semibold ${on ? 'text-accent-ink' : 'text-fg'}`}>{p.label}</span>
                  <span className="block truncate text-2xs text-muted">{p.hint}</span>
                </span>
              </button>
            )
          })}
        </div>

        {showBaseUrl && (
          <Field label="Base URL">
            <TextInput
              icon={<LinkIcon size={14} />}
              name="format-base-url"
              autoComplete="off"
              spellCheck={false}
              value={baseUrl}
              onChange={(e) => { setBaseUrl(e.target.value); setTestResult('idle') }}
              placeholder={preset.baseUrl || 'http://localhost:11434/v1'}
              className="font-mono"
            />
          </Field>
        )}

        <Field label="Model">
          <TextInput
            icon={<CubeIcon size={14} />}
            name="format-model"
            autoComplete="off"
            spellCheck={false}
            value={model}
            onChange={(e) => { setModel(e.target.value); setTestResult('idle') }}
            placeholder={preset.modelHint}
            className="font-mono"
          />
          <span className="text-2xs text-muted">
            Pick a small instruct model, not a reasoning one. For example <span className="font-mono">{preset.modelHint}</span>
          </span>
        </Field>

        <Field label={preset.needsKey ? 'API key' : 'API key (optional)'}>
          <TextInput
            icon={<KeyIcon size={14} />}
            name="format-api-key"
            type="password"
            autoComplete="off"
            spellCheck={false}
            value={apiKey}
            onChange={(e) => { setApiKey(e.target.value); setTestResult('idle') }}
            placeholder={keySaved && !removeKey
              ? 'Saved in your system keychain — type to replace'
              : preset.needsKey ? 'sk-…' : 'Leave blank for local servers'}
            className="font-mono placeholder:font-sans"
          />
          {keySaved && !removeKey && apiKey === '' && (
            <button
              type="button"
              onClick={() => { setRemoveKey(true); setTestResult('idle') }}
              className="cursor-pointer self-start text-2xs text-muted hover:text-danger"
            >
              Remove saved key
            </button>
          )}
        </Field>
      </div>

      <div className="flex items-center justify-end gap-2 border-t border-line-soft px-6 py-3.5">
        <span className="mr-auto flex items-center gap-1.5 text-2xs text-muted">
          <LockSimpleIcon size={12} />
          Stored on this device only
        </span>
        <Button variant="outline" onClick={handleTest} disabled={!canSubmit || testing}>
          {testing ? <SpinnerGapIcon size={13} weight="bold" className="animate-spin" />
            : testResult === 'ok' ? <CheckCircleIcon size={13} weight="fill" className="text-success" />
            : testResult === 'fail' ? <WarningCircleIcon size={13} weight="fill" className="text-danger" />
            : <PlugIcon size={13} weight="bold" />}
          Test connection
        </Button>
        <Button variant="ghost" onClick={onClose}>Cancel</Button>
        <Button variant="primary" onClick={handleSave} disabled={!canSubmit || saving}>
          {saving ? <SpinnerGapIcon size={13} weight="bold" className="animate-spin" /> : <CheckIcon size={13} weight="bold" />}
          {saving ? 'Saving…' : 'Save & enable'}
        </Button>
      </div>
    </>
  )
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="flex flex-col gap-1.5">
      <span className="text-xs font-semibold text-fg-2">{label}</span>
      {children}
    </label>
  )
}
