import { useState, useEffect, useCallback, useRef } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { motion, AnimatePresence } from 'framer-motion'
import { Sparkles, Settings2, X, CheckCircle2, Loader2, AlertCircle, Plug, Save } from 'lucide-react'
import { COMMANDS } from '../lib/commands'
import { extractErrorMessage } from '../lib/errors'
import { toast } from 'sonner'

type FormatConfig = {
  enabled: boolean
  provider: string
  baseUrl: string
  model: string
  apiKey: string
}

type Preset = {
  id: string
  label: string
  baseUrl: string
  needsKey: boolean
  modelHint: string
}

// All providers speak the OpenAI /chat/completions API; presets only prefill
// the base URL. Key + model are always user-entered. "Custom" prefills nothing.
const PRESETS: Preset[] = [
  { id: 'ollama',     label: 'Ollama (local/cloud)', baseUrl: 'http://localhost:11434/v1', needsKey: false, modelHint: 'qwen2.5:3b-instruct' },
  { id: 'lmstudio',   label: 'LM Studio (local)', baseUrl: 'http://localhost:1234/v1',  needsKey: false, modelHint: 'qwen2.5-3b-instruct' },
  { id: 'openai',     label: 'OpenAI',            baseUrl: 'https://api.openai.com/v1',  needsKey: true,  modelHint: 'gpt-5.5' },
  { id: 'openrouter', label: 'OpenRouter',        baseUrl: 'https://openrouter.ai/api/v1', needsKey: true, modelHint: 'meta-llama/llama-3.1-8b-instruct' },
  { id: 'custom',     label: 'Custom',            baseUrl: '',                            needsKey: false, modelHint: 'model name' },
]

const DEFAULT_CONFIG: FormatConfig = { enabled: false, provider: 'ollama', baseUrl: '', model: '', apiKey: '' }

const INPUT_CLASS =
  'w-full px-3 py-2 text-[12px] text-(--fg) bg-(--surface) border border-(--border) rounded-(--r-md) outline-none transition-colors duration-(--t-fast) focus:border-(--accent) placeholder:text-(--muted)'

/**
 * Smart-formatting toggle + provider configuration. The formatter is an
 * OpenAI-compatible HTTP endpoint (local Ollama, OpenAI, OpenRouter, or custom).
 * Off by default. Enabling without a configured endpoint prompts the user to
 * configure one.
 */
export function FormattingToggle() {
  const [config, setConfig] = useState<FormatConfig>(DEFAULT_CONFIG)
  const [modalOpen, setModalOpen] = useState(false)

  const refresh = useCallback(() => {
    invoke<FormatConfig>(COMMANDS.GET_FORMAT_CONFIG).then(setConfig).catch(() => {})
  }, [])

  useEffect(() => { refresh() }, [refresh])

  const configured = config.baseUrl.trim() !== '' && config.model.trim() !== ''

  const persist = async (next: FormatConfig) => {
    setConfig(next)
    await invoke(COMMANDS.SET_FORMAT_CONFIG, { config: next }).catch(() => {
      toast.error('Failed to save formatting settings')
    })
  }

  const toggle = async () => {
    const next = !config.enabled
    if (next && !configured) {
      // Can't enable without an endpoint — open the config modal instead.
      setModalOpen(true)
      return
    }
    await persist({ ...config, enabled: next })
  }

  return (
    <div className="flex flex-col gap-2 pt-2 border-t border-[var(--border-soft)]">
      <div className="flex items-center justify-between gap-4">
        <div className="flex items-start gap-3">
          <div className="w-8 h-8 mt-px rounded-[var(--r-md)] bg-[var(--accent-soft)] text-[var(--accent)] flex items-center justify-center shrink-0">
            <Sparkles size={14} strokeWidth={2} />
          </div>
          <div>
            <p className="text-[12px] font-semibold text-[var(--fg-2)] tracking-[-0.01em]">Smart formatting</p>
            <p className="text-[11px] text-[var(--muted)] mt-[3px] max-w-[340px]">
              Sends each transcript to your chosen LLM (local Ollama, OpenAI, OpenRouter…)
              to clean up punctuation and turn spoken lists into real lists before pasting.
            </p>
          </div>
        </div>

        <button
          type="button"
          role="switch"
          aria-checked={config.enabled}
          aria-label="Toggle smart formatting"
          onClick={toggle}
          className="relative w-[42px] h-[24px] rounded-full shrink-0 cursor-pointer border-none p-0 transition-colors duration-(--t-normal)"
          style={{ backgroundColor: config.enabled ? 'var(--accent)' : 'var(--border)' }}
        >
          <motion.span
            className="absolute top-[3px] left-[3px] w-[18px] h-[18px] rounded-full bg-white shadow-sm"
            animate={{ x: config.enabled ? 18 : 0 }}
            transition={{ type: 'spring', stiffness: 400, damping: 28 }}
          />
        </button>
      </div>

      {/* Configure row */}
      <div className="flex items-center justify-between gap-3 pl-11">
        <span className="text-[10px] text-[var(--muted)]">
          {configured
            ? `${config.provider === 'custom' ? 'Custom' : config.provider} · ${config.model}`
            : 'No endpoint configured'}
        </span>
        <button
          type="button"
          onClick={() => setModalOpen(true)}
          className="flex items-center gap-1.5 text-[11px] font-medium text-[var(--accent)] bg-transparent border-none cursor-pointer hover:underline"
        >
          <Settings2 size={12} strokeWidth={1.75} />
          Configure
        </button>
      </div>

      <AnimatePresence>
        {modalOpen && (
          <ProviderModal
            initial={config}
            onClose={() => setModalOpen(false)}
            onSaved={(saved) => { setConfig(saved); setModalOpen(false) }}
          />
        )}
      </AnimatePresence>
    </div>
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
  const [baseUrl, setBaseUrl] = useState(initial.baseUrl)
  const [model, setModel] = useState(initial.model)
  const [apiKey, setApiKey] = useState(initial.apiKey)
  const [testing, setTesting] = useState(false)
  const [testResult, setTestResult] = useState<'idle' | 'ok' | 'fail'>('idle')
  const [saving, setSaving] = useState(false)
  const resultTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const preset = PRESETS.find((p) => p.id === providerId) ?? PRESETS[0]

  // Close on Escape
  useEffect(() => {
    const handler = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose() }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [onClose])

  // Clean up the result-flash timer on unmount.
  useEffect(() => () => { if (resultTimerRef.current) clearTimeout(resultTimerRef.current) }, [])

  const selectProvider = (id: string) => {
    setProviderId(id)
    setTestResult('idle')
    const p = PRESETS.find((x) => x.id === id)
    // Prefill base URL from the preset (custom clears it). Leave model/key as-is
    // so switching presets doesn't wipe what the user typed.
    if (p && p.id !== 'custom') setBaseUrl(p.baseUrl)
    else if (p && p.id === 'custom') setBaseUrl('')
  }

  const draft = (): FormatConfig => ({
    enabled: initial.enabled,
    provider: providerId,
    baseUrl: baseUrl.trim(),
    model: model.trim(),
    apiKey: apiKey.trim(),
  })

  const canSubmit = baseUrl.trim() !== '' && model.trim() !== ''

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
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-[2px]"
      onClick={(e) => { if (e.target === e.currentTarget) onClose() }}
    >
      <motion.div
        initial={{ opacity: 0, scale: 0.96, y: 8 }}
        animate={{ opacity: 1, scale: 1, y: 0 }}
        exit={{ opacity: 0, scale: 0.96, y: 8 }}
        transition={{ duration: 0.2, ease: 'easeOut' }}
        className="w-[460px] flex flex-col bg-(--panel) border border-(--border) rounded-(--r-xl) shadow-(--shadow-lg) overflow-hidden"
      >
        {/* Header */}
        <div className="flex items-center justify-between px-6 pt-6 pb-4 border-b border-(--border-soft)">
          <div className="flex items-center gap-3">
            <div className="w-9 h-9 rounded-(--r-lg) bg-(--accent-soft) text-(--accent) flex items-center justify-center shrink-0">
              <Sparkles size={15} strokeWidth={2} />
            </div>
            <div>
              <h2 className="text-[15px] font-bold tracking-tight text-(--fg) m-0">Formatting provider</h2>
              <p className="text-[11px] text-muted-foreground mt-0.5">Connect any OpenAI-compatible endpoint</p>
            </div>
          </div>
          <button
            type="button"
            aria-label="Close"
            className="flex items-center justify-center w-7 h-7 rounded-(--r-md) text-muted-foreground bg-transparent border-none cursor-pointer transition-[color,background] duration-(--t-fast) hover:text-(--fg) hover:bg-accent"
            onClick={onClose}
          >
            <X size={14} strokeWidth={2} />
          </button>
        </div>

        <div className="flex flex-col gap-4 px-6 py-5">
          {/* Provider presets */}
          <div className="grid grid-cols-2 gap-2">
            {PRESETS.map((p) => {
              const active = p.id === providerId
              return (
                <button
                  key={p.id}
                  type="button"
                  onClick={() => selectProvider(p.id)}
                  className="flex items-center justify-center px-3 py-2 rounded-(--r-md) border-[1.5px] text-[12px] font-medium cursor-pointer transition-colors duration-(--t-fast)"
                  style={{
                    backgroundColor: active ? 'var(--accent-soft)' : 'var(--surface)',
                    borderColor: active ? 'var(--accent)' : 'var(--border)',
                    color: active ? 'var(--accent)' : 'var(--fg)',
                  }}
                >
                  {p.label}
                </button>
              )
            })}
          </div>

          {/* Base URL */}
          <Field label="Base URL">
            <input
              type="text"
              value={baseUrl}
              onChange={(e) => { setBaseUrl(e.target.value); setTestResult('idle') }}
              placeholder="http://localhost:11434/v1"
              className={INPUT_CLASS}
              spellCheck={false}
            />
          </Field>

          {/* Model */}
          <Field label="Model">
            <input
              type="text"
              value={model}
              onChange={(e) => { setModel(e.target.value); setTestResult('idle') }}
              placeholder={preset.modelHint}
              className={INPUT_CLASS}
              spellCheck={false}
            />
            <span className="text-[10px] text-(--muted)">
              Tip: use a small instruct model (e.g. qwen2.5-3b-instruct), not a reasoning model — much faster and more accurate for formatting.
            </span>
          </Field>

          {/* API key */}
          <Field label={preset.needsKey ? 'API key' : 'API key (optional)'}>
            <input
              type="password"
              value={apiKey}
              onChange={(e) => { setApiKey(e.target.value); setTestResult('idle') }}
              placeholder={preset.needsKey ? 'sk-…' : 'leave blank for local servers'}
              className={INPUT_CLASS}
              spellCheck={false}
            />
          </Field>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between gap-3 px-6 pb-5 pt-1">
          <button
            type="button"
            onClick={handleTest}
            disabled={!canSubmit || testing}
            className="flex items-center gap-2 text-[12px] font-medium text-(--fg-2) bg-(--surface) border border-(--border) rounded-(--r-md) px-3 py-2 cursor-pointer transition-colors duration-(--t-fast) hover:bg-(--surface-hover) disabled:opacity-40 disabled:cursor-not-allowed"
          >
            {testing ? <Loader2 size={13} className="animate-spin" />
              : testResult === 'ok' ? <CheckCircle2 size={13} className="text-(--success)" />
              : testResult === 'fail' ? <AlertCircle size={13} className="text-(--destructive)" />
              : <Plug size={13} strokeWidth={1.75} />}
            Test connection
          </button>
          <button
            type="button"
            onClick={handleSave}
            disabled={!canSubmit || saving}
            className="flex items-center gap-2 text-[12px] font-semibold text-(--accent-fg) bg-(--accent) border-none rounded-(--r-md) px-4 py-2 cursor-pointer transition-opacity duration-(--t-fast) hover:opacity-90 disabled:opacity-40 disabled:cursor-not-allowed"
          >
            {saving ? <Loader2 size={13} className="animate-spin" /> : <Save size={13} strokeWidth={1.75} />}
            {saving ? 'Saving…' : 'Save & enable'}
          </button>
        </div>
      </motion.div>
    </div>
  )
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="flex flex-col gap-1.5">
      <span className="text-[11px] font-medium text-(--fg-2)">{label}</span>
      {children}
    </label>
  )
}
