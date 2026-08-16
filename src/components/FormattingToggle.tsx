import { useState, useEffect, useCallback, useRef } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { motion, AnimatePresence } from 'framer-motion'
import { Dialog } from 'radix-ui'
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

// Every preset but "anthropic" speaks the OpenAI-compatible API; presets only
// prefill the base URL, and "anthropic"/"custom" prefill nothing.
const PRESETS: Preset[] = [
  { id: 'ollama',     label: 'Ollama (local/cloud)', baseUrl: 'http://localhost:11434/v1', needsKey: false, modelHint: 'qwen2.5:3b-instruct' },
  { id: 'lmstudio',   label: 'LM Studio (local)', baseUrl: 'http://localhost:1234/v1',  needsKey: false, modelHint: 'qwen2.5-3b-instruct' },
  { id: 'openai',     label: 'OpenAI',            baseUrl: 'https://api.openai.com/v1',  needsKey: true,  modelHint: 'gpt-5.5' },
  { id: 'openrouter', label: 'OpenRouter',        baseUrl: 'https://openrouter.ai/api/v1', needsKey: true, modelHint: 'meta-llama/llama-3.1-8b-instruct' },
  { id: 'anthropic',  label: 'Anthropic',         baseUrl: '',                            needsKey: true,  modelHint: 'claude-sonnet-5' },
  { id: 'custom',     label: 'Custom',            baseUrl: '',                            needsKey: false, modelHint: 'model name' },
]

const DEFAULT_CONFIG: FormatConfig = { enabled: false, provider: 'ollama', baseUrl: '', model: '', apiKey: '' }

const INPUT_CLASS =
  'w-full px-3 py-2 text-[12px] text-(--fg) bg-(--surface) border border-(--border) rounded-(--r-md) outline-none transition-colors duration-(--t-fast) focus:border-(--accent) placeholder:text-(--muted)'

/**
 * Smart-formatting toggle + provider configuration. The formatter is an HTTP
 * call (Ollama, LM Studio, OpenAI, OpenRouter, Anthropic, or custom). Off by
 * default. Enabling without a configured endpoint prompts the user to
 * configure one.
 */
export function FormattingToggle() {
  const [config, setConfig] = useState<FormatConfig>(DEFAULT_CONFIG)
  const [modalOpen, setModalOpen] = useState(false)

  const refresh = useCallback(() => {
    invoke<FormatConfig>(COMMANDS.GET_FORMAT_CONFIG).then(setConfig).catch(() => {})
  }, [])

  useEffect(() => { refresh() }, [refresh])

  const configured =
    config.model.trim() !== '' &&
    (config.provider === 'anthropic' || config.baseUrl.trim() !== '')

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

  const presetLabel = PRESETS.find((p) => p.id === config.provider)?.label ?? config.provider

  return (
    <div className="overflow-hidden rounded-(--r-lg) border border-(--border-soft) bg-(--panel)">
      <div className="px-4 py-2.5 border-b border-(--border-soft) text-[10px] font-semibold uppercase tracking-[0.08em] text-(--muted)">
        Smart formatting
      </div>

      <div className="flex items-center justify-between gap-4 p-4">
        <div className="flex items-start gap-3">
          <div className="w-8 h-8 mt-px rounded-(--r-md) bg-(--accent-soft) text-(--accent) flex items-center justify-center shrink-0">
            <Sparkles size={14} strokeWidth={2} />
          </div>
          <div>
            <p className="text-[12px] font-semibold text-(--fg-2) tracking-[-0.01em]">
              {configured ? `${presetLabel} · ${config.model}` : 'No endpoint configured'}
            </p>
            <p className="text-[11px] text-(--muted) mt-[3px] max-w-[380px]">
              Sends each transcript to your chosen LLM (local Ollama, OpenAI, Anthropic, OpenRouter…)
              to clean up punctuation and turn spoken lists into real lists before pasting.
            </p>
          </div>
        </div>

        <div className="flex items-center gap-2 shrink-0">
          <motion.button
            type="button"
            onClick={() => setModalOpen(true)}
            className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-(--r-md) border border-(--border) bg-(--surface) text-[12px] font-medium text-(--fg-2) cursor-pointer"
            whileHover={{ backgroundColor: 'var(--surface-hover)', color: 'var(--fg)' }}
            whileTap={{ scale: 0.96 }}
            transition={{ duration: 0.15 }}
          >
            <Settings2 size={12} strokeWidth={1.75} />
            Configure
          </motion.button>
          <motion.button
            type="button"
            role="switch"
            aria-checked={config.enabled}
            aria-label="Toggle smart formatting"
            onClick={toggle}
            className="relative w-[42px] h-[24px] rounded-full shrink-0 cursor-pointer border-none p-0"
            initial={false}
            animate={{ backgroundColor: config.enabled ? 'var(--accent)' : 'var(--border)' }}
            whileTap={{ scale: 0.95 }}
            transition={{ duration: 0.2 }}
          >
            <motion.span
              className="absolute top-[3px] left-[3px] w-[18px] h-[18px] rounded-full bg-white shadow-sm"
              animate={{ x: config.enabled ? 18 : 0 }}
              transition={{ type: 'spring', stiffness: 400, damping: 28 }}
            />
          </motion.button>
        </div>
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
  // Anthropic pins its own endpoint (native Messages API) — a base URL field
  // would be misleading, so it's hidden entirely for that provider.
  const showBaseUrl = providerId !== 'anthropic'

  // Clean up the result-flash timer on unmount.
  useEffect(() => () => { if (resultTimerRef.current) clearTimeout(resultTimerRef.current) }, [])

  const selectProvider = (id: string) => {
    setProviderId(id)
    setTestResult('idle')
    const p = PRESETS.find((x) => x.id === id)
    // Prefill base URL from the preset (custom/anthropic clear it). Leave
    // model/key as-is so switching presets doesn't wipe what the user typed.
    if (p && p.id !== 'custom' && p.id !== 'anthropic') setBaseUrl(p.baseUrl)
    else setBaseUrl('')
  }

  const draft = (): FormatConfig => ({
    enabled: initial.enabled,
    provider: providerId,
    baseUrl: baseUrl.trim(),
    model: model.trim(),
    apiKey: apiKey.trim(),
  })

  const canSubmit =
    model.trim() !== '' &&
    (!preset.needsKey || apiKey.trim() !== '') &&
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
      <Dialog.Portal forceMount>
        <Dialog.Overlay asChild>
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.18 }}
            className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-[2px]"
          />
        </Dialog.Overlay>
        <Dialog.Content
          aria-describedby={undefined}
          asChild
          onOpenAutoFocus={(e) => e.preventDefault()}
          className="fixed inset-0 z-50 flex items-center justify-center pointer-events-none"
        >
          <div>
      <motion.div
        initial={{ opacity: 0, scale: 0.96, y: 8 }}
        animate={{ opacity: 1, scale: 1, y: 0 }}
        exit={{ opacity: 0, scale: 0.96, y: 8 }}
        transition={{ duration: 0.2, ease: 'easeOut' }}
        className="w-[460px] flex flex-col bg-(--panel) border border-(--border) rounded-(--r-xl) shadow-(--shadow-lg) overflow-hidden pointer-events-auto"
      >
        {/* Header */}
        <div className="flex items-center justify-between px-6 pt-6 pb-4 border-b border-(--border-soft)">
          <div className="flex items-center gap-3">
            <div className="w-9 h-9 rounded-(--r-lg) bg-(--accent-soft) text-(--accent) flex items-center justify-center shrink-0">
              <Sparkles size={15} strokeWidth={2} />
            </div>
            <div>
              <Dialog.Title className="text-[15px] font-bold tracking-tight text-(--fg) m-0">Formatting provider</Dialog.Title>
              <p className="text-[11px] text-muted-foreground mt-0.5">Connect Anthropic or any OpenAI-compatible endpoint</p>
            </div>
          </div>
          <Dialog.Close asChild>
            <motion.button
              type="button"
              aria-label="Close"
              className="flex items-center justify-center w-7 h-7 rounded-(--r-md) text-muted-foreground bg-transparent border-none cursor-pointer"
              whileHover={{ backgroundColor: 'var(--surface-hover)', color: 'var(--fg)' }}
              whileTap={{ scale: 0.92 }}
              transition={{ duration: 0.15 }}
            >
              <X size={14} strokeWidth={2} />
            </motion.button>
          </Dialog.Close>
        </div>

        <div className="flex flex-col gap-4 px-6 py-5">
          {/* Provider presets */}
          <div className="grid grid-cols-2 gap-2">
            {PRESETS.map((p) => {
              const active = p.id === providerId
              return (
                <motion.button
                  key={p.id}
                  type="button"
                  onClick={() => selectProvider(p.id)}
                  className="flex items-center justify-center px-3 py-2 rounded-(--r-md) border-[1.5px] text-[12px] font-medium cursor-pointer"
                  initial={false}
                  animate={{
                    backgroundColor: active ? 'var(--accent-soft)' : 'var(--surface)',
                    borderColor: active ? 'var(--accent)' : 'var(--border)',
                    color: active ? 'var(--accent)' : 'var(--fg)',
                  }}
                  whileHover={active ? undefined : { backgroundColor: 'var(--surface-hover)' }}
                  whileTap={{ scale: 0.97 }}
                  transition={{ type: 'spring', stiffness: 300, damping: 25, mass: 0.8 }}
                >
                  {p.label}
                </motion.button>
              )
            })}
          </div>

          {/* Base URL */}
          {showBaseUrl ? (
            <Field label="Base URL">
              <input
                id="format-base-url"
                name="format-base-url"
                type="text"
                autoComplete="off"
                value={baseUrl}
                onChange={(e) => { setBaseUrl(e.target.value); setTestResult('idle') }}
                placeholder={preset.baseUrl || 'http://localhost:11434/v1'}
                className={INPUT_CLASS}
                spellCheck={false}
              />
            </Field>
          ) : null}

          {/* Model */}
          <Field label="Model">
            <input
              id="format-model"
              name="format-model"
              type="text"
              autoComplete="off"
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
              id="format-api-key"
              name="format-api-key"
              type="password"
              autoComplete="off"
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
          <motion.button
            type="button"
            onClick={handleTest}
            disabled={!canSubmit || testing}
            className="flex items-center gap-2 text-[12px] font-medium text-(--fg-2) bg-(--surface) border border-(--border) rounded-(--r-md) px-3 py-2 cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed"
            whileHover={!canSubmit || testing ? undefined : { backgroundColor: 'var(--surface-hover)' }}
            whileTap={!canSubmit || testing ? undefined : { scale: 0.97 }}
            transition={{ duration: 0.15 }}
          >
            {testing ? <Loader2 size={13} className="animate-spin" />
              : testResult === 'ok' ? <CheckCircle2 size={13} className="text-(--success)" />
              : testResult === 'fail' ? <AlertCircle size={13} className="text-(--destructive)" />
              : <Plug size={13} strokeWidth={1.75} />}
            Test connection
          </motion.button>
          <motion.button
            type="button"
            onClick={handleSave}
            disabled={!canSubmit || saving}
            className="flex items-center gap-2 text-[12px] font-semibold text-(--accent-fg) bg-(--accent) border-none rounded-(--r-md) px-4 py-2 cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed"
            whileHover={!canSubmit || saving ? undefined : { opacity: 0.9 }}
            whileTap={!canSubmit || saving ? undefined : { scale: 0.97 }}
            transition={{ duration: 0.15 }}
          >
            {saving ? <Loader2 size={13} className="animate-spin" /> : <Save size={13} strokeWidth={1.75} />}
            {saving ? 'Saving…' : 'Save & enable'}
          </motion.button>
        </div>
      </motion.div>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
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
