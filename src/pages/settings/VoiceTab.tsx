import { useState, useCallback, useEffect, useMemo } from 'react'
import { AnimatePresence, motion } from 'framer-motion'
import { invoke } from '@tauri-apps/api/core'
import { COMMANDS } from '../../lib/commands'
import {
  formatModelSize, modelNameToId, pipelineLabel, sortForDisplay,
  type CatalogModel, type ModelId,
} from '../../lib/models'
import { toast } from 'sonner'
import {
  CheckCircle2, Download, Cpu, Database, HardDrive, X,
  Radio, Wind, Server, Layers, Gem, Loader2,
} from 'lucide-react'
import { FormattingToggle } from '../../components/FormattingToggle'
import { MicrophoneSection } from './MicrophoneSection'
import { LanguageSection } from './LanguageSection'
import { ModelManagerModal } from '../../components/ModelManagerModal'
import type { HardwareProfile } from '../../types'
import { useAppStore } from '../../store/useAppStore'

/** Which icon a catalog entry gets: family first, then capability tier. */
type IconKey = 'stream' | 'light' | 'standard' | 'balanced' | 'heavy' | 'none'

const FAMILY_ICON_KEYS: Record<string, IconKey> = {
  parakeet: 'stream',
  nemotron: 'stream',
  moonshine: 'light',
  qwen3asr: 'balanced',
  canary: 'standard',
}

function iconKey(model: CatalogModel | null | undefined): IconKey {
  if (!model) return 'none'
  const byFamily = FAMILY_ICON_KEYS[model.family.replace(/-/g, '')]
  if (byFamily) return byFamily
  if (model.sizeBytes >= 1_000_000_000) return 'heavy'
  if (model.sizeBytes >= 500_000_000) return 'balanced'
  if (model.sizeBytes >= 150_000_000) return 'standard'
  return 'light'
}

/** Renders a catalog entry's icon from a fixed set, so no component type is
 *  constructed during render. */
function ModelIcon({ model, size }: { model: CatalogModel | null | undefined; size: number }) {
  const props = { size, strokeWidth: 1.75 }
  switch (iconKey(model)) {
    case 'stream': return <Radio {...props} />
    case 'light': return <Wind {...props} />
    case 'standard': return <Server {...props} />
    case 'balanced': return <Layers {...props} />
    case 'heavy': return <Gem {...props} />
    default: return <Cpu {...props} />
  }
}

type DownloadedModel = {
  variant: string
  displayName: string
  sizeBytes: number
  isActive: boolean
}

/** Shared shell for a model row's right-side status pill, so Loaded / Installed
 *  / Download / downloading all read as one consistent control. */
const STATE_PILL = 'flex items-center justify-center gap-1.5 h-6 min-w-24 px-2 rounded-(--r-md) border text-[11px] tracking-[-0.01em] transition-colors duration-(--t-fast)'

/** One model row: icon + name + trait on the left, the action on the right.
 *  The pill is the only click target — Download / Use / Loaded. */
function ModelRow({
  icon,
  name,
  fullName,
  trait,
  pipeline,
  sizeLabel,
  loaded,
  recommended,
  installed,
  downloading,
  downloadPct,
  disabled,
  onSelect,
  onCancel,
}: {
  icon: React.ReactNode
  name: string
  fullName: string
  trait: string
  /** Badge text, or null when the model has nothing to call out. */
  pipeline: string | null
  sizeLabel: string
  loaded: boolean
  recommended: boolean
  installed: boolean
  downloading: boolean
  downloadPct: number
  disabled: boolean
  onSelect: () => void
  onCancel: () => void
}) {
  const inert = disabled && !downloading
  // Which state pill is showing — also the AnimatePresence key, so swapping
  // state cross-fades instead of popping.
  const pill = downloading ? 'downloading' : loaded ? 'loaded' : installed ? 'installed' : 'download'
  return (
    <motion.div
      title={fullName}
      className="flex items-center gap-3 w-full px-2 py-2 rounded-(--r-md) text-left"
      initial={false}
      animate={{
        backgroundColor: loaded ? 'var(--accent-soft)' : 'rgba(0,0,0,0)',
        opacity: inert ? 0.5 : 1,
      }}
      transition={{ type: 'spring', stiffness: 300, damping: 25, mass: 0.8 }}
    >
      <motion.span
        className="grid place-items-center shrink-0 size-8 rounded-(--r-md)"
        initial={false}
        animate={{
          backgroundColor: loaded ? 'var(--accent-soft)' : 'var(--surface)',
          color: loaded ? 'var(--accent)' : 'var(--fg-2)',
        }}
        transition={{ duration: 0.2 }}
      >
        {icon}
      </motion.span>

      <span className="flex-1 min-w-0">
        <span className="flex items-center gap-2">
          <motion.span
            className="text-[12.5px] font-semibold tracking-[-0.01em]"
            initial={false}
            animate={{ color: loaded ? 'var(--accent)' : 'var(--fg)' }}
            transition={{ duration: 0.2 }}
          >
            {name}
          </motion.span>
          <AnimatePresence initial={false}>
            {recommended && !loaded ? (
              <motion.span
                key="recommended"
                initial={{ opacity: 0, scale: 0.9 }}
                animate={{ opacity: 1, scale: 1 }}
                exit={{ opacity: 0, scale: 0.9 }}
                transition={{ duration: 0.15 }}
                className="text-[9px] font-semibold uppercase tracking-[0.04em] text-(--accent) bg-(--accent-soft) border border-(--accent-soft) rounded-(--r-xs) px-1.5 py-px"
              >
                Recommended
              </motion.span>
            ) : null}
          </AnimatePresence>
          {pipeline && (
            <span className="text-[9px] font-semibold uppercase tracking-[0.04em] rounded-(--r-xs) px-1.5 py-px border text-(--accent) bg-(--accent-soft) border-(--accent-soft)">
              {pipeline}
            </span>
          )}
        </span>
        <span className="block mt-0.5 text-[11px] text-muted-foreground truncate">
          {trait} · {sizeLabel}
        </span>
      </span>

      <span className="flex items-center gap-1.5 shrink-0">
        <AnimatePresence mode="wait" initial={false}>
          <motion.span
            key={pill}
            initial={{ opacity: 0, scale: 0.94 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.94 }}
            transition={{ duration: 0.15 }}
            className="flex items-center gap-1.5"
          >
            {downloading ? (
              <>
                <span className={`${STATE_PILL} border-(--accent) bg-(--accent-soft) text-(--accent)`}>
                  <motion.span
                    className="flex"
                    animate={{ rotate: 360 }}
                    transition={{ duration: 1, ease: 'linear', repeat: Infinity }}
                  >
                    <Loader2 size={12} strokeWidth={2} />
                  </motion.span>
                  <span className="tabular-nums">{downloadPct}%</span>
                </span>
                <motion.button
                  type="button"
                  aria-label="Cancel download"
                  title="Cancel download"
                  onClick={onCancel}
                  className="grid place-items-center size-6 rounded-(--r-md) border border-(--border) bg-(--surface) text-muted-foreground cursor-pointer"
                  whileHover={{ backgroundColor: 'var(--surface-hover)', color: 'var(--danger)' }}
                  whileTap={{ scale: 0.92 }}
                  transition={{ duration: 0.15 }}
                >
                  <X size={12} strokeWidth={2} />
                </motion.button>
              </>
            ) : loaded ? (
              <span className={`${STATE_PILL} border-(--accent) bg-(--accent-soft) text-(--accent) font-semibold`}>
                <CheckCircle2 size={12} strokeWidth={2.25} />
                Loaded
              </span>
            ) : installed ? (
              <motion.button
                type="button"
                disabled={inert}
                onClick={onSelect}
                className={`${STATE_PILL} border-(--border) bg-(--surface) text-(--fg-2) ${
                  inert ? 'cursor-not-allowed' : 'cursor-pointer'
                }`}
                whileHover={inert ? undefined : { backgroundColor: 'var(--surface-hover)', color: 'var(--fg)' }}
                whileTap={inert ? undefined : { scale: 0.95 }}
                transition={{ duration: 0.15 }}
              >
                <HardDrive size={11} strokeWidth={1.75} />
                Use
              </motion.button>
            ) : (
              <motion.button
                type="button"
                disabled={inert}
                onClick={onSelect}
                className={`${STATE_PILL} border-(--border) bg-(--surface) text-(--fg-2) ${
                  inert ? 'cursor-not-allowed' : 'cursor-pointer'
                }`}
                whileHover={inert ? undefined : { backgroundColor: 'var(--surface-hover)', color: 'var(--fg)' }}
                whileTap={inert ? undefined : { scale: 0.95 }}
                transition={{ duration: 0.15 }}
              >
                <Download size={11} strokeWidth={2} />
                Download
              </motion.button>
            )}
          </motion.span>
        </AnimatePresence>
      </span>
    </motion.div>
  )
}

export function VoiceTab() {
  const [profile, setProfile] = useState<HardwareProfile | null>(null)
  const [modelSaving, setModelSaving] = useState(false)
  const [onDisk, setOnDisk] = useState<DownloadedModel[]>([])
  const [managerOpen, setManagerOpen] = useState(false)

  const catalog = useAppStore(s => s.catalog)
  const orderedModels = useMemo(() => sortForDisplay(catalog), [catalog])
  const refreshCatalog = useAppStore(s => s.refreshCatalog)
  const modelDownloading = useAppStore(s => s.modelDownloading)
  const modelDownloadPct = useAppStore(s => s.downloadProgress)
  const selected = useAppStore(s => s.selectedModel)
  const setDownloadingFromModel = useAppStore(s => s.setDownloadingFromModel)
  const setSelectedModel = useAppStore(s => s.setSelectedModel)
  const refreshModelInfo = useAppStore(s => s.refreshModelInfo)
  const cancelDownload = useAppStore(s => s.cancelDownload)
  // Drives the input row's column count — a lone mic picker takes the full width.
  const [langSupported, setLangSupported] = useState(false)

  const refreshOnDisk = useCallback(() => {
    invoke<DownloadedModel[]>(COMMANDS.GET_DOWNLOADED_MODELS).then(setOnDisk).catch(() => setOnDisk([]))
  }, [])

  useEffect(() => {
    invoke<HardwareProfile>(COMMANDS.GET_HARDWARE_PROFILE).then(setProfile).catch(() => {})
    void refreshModelInfo()
    void refreshCatalog()
    refreshOnDisk()
  }, [refreshModelInfo, refreshCatalog, refreshOnDisk])

  // Re-read on-disk state once a download finishes/cancels.
  useEffect(() => {
    if (!modelDownloading) refreshOnDisk()
  }, [modelDownloading, refreshOnDisk])

  const onDiskVariants = new Set(onDisk.map(m => m.variant))
  const recommendedVariant = profile ? modelNameToId(profile.recommendedModel, catalog) : null
  // `selected` is set even when nothing is downloaded, so "Loaded" also
  // requires the file to actually be on disk.
  const loadedVariant =
    selected && onDiskVariants.has(selected) && !modelDownloading ? selected : null

  const handleModelChange = async (v: ModelId) => {
    if (modelDownloading) return
    setDownloadingFromModel(selected ?? v)
    setSelectedModel(v)
    setModelSaving(true)
    try {
      await invoke(COMMANDS.SET_MODEL_OVERRIDE, { variant: v })
      // Already on disk means nothing to fetch — the override alone re-warms it.
      if (!onDiskVariants.has(v)) {
        invoke(COMMANDS.RETRY_MODEL_DOWNLOAD).catch(() => {})
      }
      await refreshModelInfo()
      toast.success('Model updated')
    } catch { /* ignore */ }
    finally { setModelSaving(false) }
  }

  return (
    <div className="flex flex-col gap-4">

      {/* Transcription model */}
      <div className="overflow-hidden rounded-(--r-lg) border border-(--border-soft) bg-(--panel)">
        <div className="px-4 py-2.5 border-b border-(--border-soft) text-[10px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">
          Transcription model
        </div>

        {/* Hero status strip */}
        <div className="flex items-center justify-between gap-4 px-4 py-3 border-b border-(--border-soft) bg-(--bg)/40">
          <div className="flex items-center gap-3 min-w-0">
            {(() => {
              // Identity follows the selection (so a download in progress names
              // its model); the accent styling follows what is really on disk.
              const shown = modelDownloading ? selected : loadedVariant
              const opt = shown ? catalog.find(m => m.id === shown) : null
              return (
                <>
                  <span className={`grid place-items-center shrink-0 size-9 rounded-(--r-lg) ${
                    loadedVariant ? 'bg-(--accent-soft) text-(--accent)' : 'bg-(--surface) text-muted-foreground'
                  }`}>
                    <ModelIcon model={opt} size={16} />
                  </span>
                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="text-[13px] font-semibold tracking-[-0.01em] text-(--fg) truncate">
                        {opt ? opt.displayName : 'No model loaded'}
                      </span>
                      {modelDownloading ? (
                        <span className="flex items-center gap-1 text-[10px] font-medium text-(--accent)">
                          <Loader2 size={10} strokeWidth={2} className="animate-spin" />
                          {modelDownloadPct}%
                        </span>
                      ) : null}
                    </div>
                    <div className="mt-0.5 text-[11px] text-muted-foreground truncate">
                      {profile
                        ? `${profile.gpuName} · ${profile.executionProvider.toUpperCase()}${profile.vramGb > 0 ? ` · ${profile.vramGb}GB VRAM` : ''}`
                        : 'Detecting hardware…'}
                    </div>
                  </div>
                </>
              )
            })()}
          </div>
          <motion.button
            type="button"
            onClick={() => setManagerOpen(true)}
            title="Manage downloaded models"
            className="flex items-center gap-1.5 shrink-0 px-2.5 py-1.5 rounded-(--r-md) border border-(--border) bg-(--surface) text-[12px] font-medium text-(--fg-2) cursor-pointer"
            whileHover={{ backgroundColor: 'var(--surface-hover)', color: 'var(--fg)' }}
            whileTap={{ scale: 0.96 }}
            transition={{ duration: 0.15 }}
          >
            <Database size={12} strokeWidth={1.75} />
            Manage
          </motion.button>
        </div>

        {/* Model list */}
        <div className="px-2 py-2">
          {orderedModels.map(model => {
            return (
              <ModelRow
                key={model.id}
                icon={<ModelIcon model={model} size={15} />}
                name={model.displayName}
                fullName={model.displayName}
                trait={model.description}
                pipeline={pipelineLabel(model.pipelines)}
                sizeLabel={formatModelSize(model.sizeBytes)}
                loaded={loadedVariant === model.id}
                recommended={recommendedVariant === model.id}
                installed={onDiskVariants.has(model.id)}
                downloading={modelDownloading && selected === model.id}
                downloadPct={modelDownloadPct}
                disabled={modelDownloading || modelSaving}
                onSelect={() => handleModelChange(model.id)}
                onCancel={cancelDownload}
              />
            )
          })}
        </div>

      </div>

      {/* Smart formatting (local LLM) */}
      <FormattingToggle />

      {/* Microphone + dictation language */}
      <div className={`grid gap-4 overflow-hidden rounded-(--r-lg) border border-(--border-soft) bg-(--panel) p-4 *:min-w-0 ${langSupported ? 'grid-cols-1 md:grid-cols-2' : 'grid-cols-1'}`}>
        <MicrophoneSection />
        <LanguageSection modelId={selected} onSupportedChange={setLangSupported} />
      </div>

      <AnimatePresence>
        {managerOpen && (
          <ModelManagerModal
            onClose={() => {
              setManagerOpen(false)
              refreshOnDisk()
              void refreshModelInfo()
            }}
          />
        )}
      </AnimatePresence>

    </div>
  )
}
