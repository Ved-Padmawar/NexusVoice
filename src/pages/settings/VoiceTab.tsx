import { useState, useCallback, useEffect } from 'react'
import { AnimatePresence, motion } from 'framer-motion'
import { invoke } from '@tauri-apps/api/core'
import { COMMANDS } from '../../lib/commands'
import { MODEL_OPTIONS, recommendedToOverride, type ModelOverride } from '../../lib/models'
import { toast } from 'sonner'
import {
  CheckCircle2, Download, Cpu, Database, HardDrive, X,
  Zap, Scale, Sparkles, Wind, Server, Layers, Box, Gem, Loader2,
} from 'lucide-react'
import { FormattingToggle } from '../../components/FormattingToggle'
import { MicrophoneSection } from './MicrophoneSection'
import { ModelManagerModal } from '../../components/ModelManagerModal'
import type { HardwareProfile } from '../../types'
import { useAppStore } from '../../store/useAppStore'
import type { BeamSize } from '../../store/uiSlice'

/** Whisper model grid — icon, label, and trait per variant, in fixed order. */
const MODEL_ROWS = [
  { value: 'tiny' as ModelOverride, Icon: Wind, label: 'Tiny', trait: 'Fastest, lowest accuracy' },
  { value: 'base' as ModelOverride, Icon: Server, label: 'Base', trait: 'Fast, basic accuracy' },
  { value: 'small' as ModelOverride, Icon: Cpu, label: 'Small', trait: 'Standard accuracy' },
  { value: 'medium' as ModelOverride, Icon: Layers, label: 'Medium', trait: 'Balanced performance' },
  { value: 'large' as ModelOverride, Icon: Box, label: 'Turbo', trait: 'High accuracy, fast' },
  { value: 'large-full' as ModelOverride, Icon: Gem, label: 'Max', trait: 'Maximum accuracy' },
] as const

/** Icon per model size — mirrors the model row list so the hero badge matches. */
const MODEL_BADGE_ICONS: Record<ModelOverride, typeof Box> = {
  tiny: Wind,
  base: Server,
  small: Cpu,
  medium: Layers,
  large: Box,
  'large-full': Gem,
}

type DownloadedModel = {
  variant: string
  displayName: string
  sizeBytes: number
  isActive: boolean
}

/** Shared shell for a model row's right-side status pill, so Loaded / Installed
 *  / Download / downloading all read as one consistent control. */
const STATE_PILL = 'flex items-center gap-1.5 h-6 px-2 rounded-(--r-md) border text-[11px] tracking-[-0.01em] transition-colors duration-(--t-fast)'

/** One selectable model row: icon + name + trait on the left, a state pill on
 *  the right. The whole row is the click target (select + auto-download);
 *  Cancel is the lone nested action and stops propagation. */
function ModelRow({
  icon,
  name,
  fullName,
  trait,
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
  const activate = () => { if (!inert) onSelect() }
  // Which state pill is showing — also the AnimatePresence key, so swapping
  // state cross-fades instead of popping.
  const pill = downloading ? 'downloading' : loaded ? 'loaded' : installed ? 'installed' : 'download'
  return (
    <motion.div
      role="button"
      tabIndex={inert ? -1 : 0}
      aria-disabled={inert}
      title={fullName}
      onClick={activate}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault()
          activate()
        }
      }}
      className={`group flex items-center gap-3 w-full px-2 py-2 rounded-(--r-md) text-left outline-none ${
        inert ? 'cursor-not-allowed' : 'cursor-pointer'
      }`}
      initial={false}
      animate={{
        backgroundColor: loaded ? 'var(--accent-soft)' : 'rgba(0,0,0,0)',
        opacity: inert ? 0.5 : 1,
      }}
      whileHover={loaded || inert ? undefined : { backgroundColor: 'var(--surface-hover)' }}
      whileTap={inert ? undefined : { scale: 0.99 }}
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
        </span>
        <span className="block mt-0.5 text-[11px] text-(--muted) truncate">{trait}</span>
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
                  onClick={(e) => { e.stopPropagation(); onCancel() }}
                  className="grid place-items-center size-6 rounded-(--r-md) border border-(--border) bg-(--surface) text-(--muted) cursor-pointer"
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
              <span className={`${STATE_PILL} border-(--border) bg-(--surface) text-(--muted)`}>
                <HardDrive size={11} strokeWidth={1.75} />
                Installed
              </span>
            ) : (
              <span className={`${STATE_PILL} border-dashed border-(--border) bg-transparent text-(--muted) group-hover:text-(--accent)`}>
                <Download size={11} strokeWidth={2} />
                Download
              </span>
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

  const beamSize = useAppStore(s => s.beamSize)
  const setBeamSize = useAppStore(s => s.setBeamSize)
  const modelDownloading = useAppStore(s => s.modelDownloading)
  const modelDownloadPct = useAppStore(s => s.downloadProgress)
  const selected = useAppStore(s => s.selectedModel)
  const setDownloadingFromModel = useAppStore(s => s.setDownloadingFromModel)
  const setSelectedModel = useAppStore(s => s.setSelectedModel)
  const refreshModelInfo = useAppStore(s => s.refreshModelInfo)
  const cancelDownload = useAppStore(s => s.cancelDownload)

  const refreshOnDisk = useCallback(() => {
    invoke<DownloadedModel[]>(COMMANDS.GET_DOWNLOADED_MODELS).then(setOnDisk).catch(() => setOnDisk([]))
  }, [])

  useEffect(() => {
    invoke<HardwareProfile>(COMMANDS.GET_HARDWARE_PROFILE).then(setProfile).catch(() => {})
    void refreshModelInfo()
    refreshOnDisk()
    invoke<number>(COMMANDS.GET_BEAM_SIZE).then(v => {
      const valid = (v === 2 || v === 5 || v === 8) ? v as BeamSize : 5
      setBeamSize(valid)
    }).catch(() => {})
  }, [setBeamSize, refreshModelInfo, refreshOnDisk])

  // Re-read on-disk state once a download finishes/cancels.
  useEffect(() => {
    if (!modelDownloading) refreshOnDisk()
  }, [modelDownloading, refreshOnDisk])

  const onDiskVariants = new Set(onDisk.map(m => m.variant))
  const recommendedVariant = profile ? recommendedToOverride(profile.recommendedModel) : null
  // `selected` is set even when nothing is downloaded, so "Loaded" also
  // requires the file to actually be on disk.
  const loadedVariant =
    selected && onDiskVariants.has(selected) && !modelDownloading ? selected : null

  const handleModelChange = async (v: ModelOverride) => {
    if (modelDownloading) return
    setDownloadingFromModel(selected ?? v)
    setSelectedModel(v)
    setModelSaving(true)
    try {
      await invoke(COMMANDS.SET_MODEL_OVERRIDE, { variant: v })
      invoke(COMMANDS.RETRY_MODEL_DOWNLOAD).catch(() => {})
      await refreshModelInfo()
      toast.success('Model updated')
    } catch { /* ignore */ }
    finally { setModelSaving(false) }
  }

  const handleBeamChange = async (v: BeamSize) => {
    setBeamSize(v)
    invoke(COMMANDS.SET_BEAM_SIZE, { beamSize: v }).catch(() => {})
  }

  return (
    <div className="flex flex-col gap-4">

      {/* Whisper model */}
      <div className="overflow-hidden rounded-(--r-lg) border border-(--border-soft) bg-(--panel)">
        <div className="px-4 py-2.5 border-b border-(--border-soft) text-[10px] font-semibold uppercase tracking-[0.08em] text-(--muted)">
          Whisper model
        </div>

        {/* Hero status strip */}
        <div className="flex items-center justify-between gap-4 px-4 py-3 border-b border-(--border-soft) bg-(--bg)/40">
          <div className="flex items-center gap-3 min-w-0">
            {(() => {
              // Identity follows the selection (so a download in progress names
              // its model); the accent styling follows what is really on disk.
              const shown = modelDownloading ? selected : loadedVariant
              const opt = shown ? MODEL_OPTIONS.find(m => m.value === shown) : null
              const ModelIcon = shown ? (MODEL_BADGE_ICONS[shown] ?? Box) : Cpu
              return (
                <>
                  <span className={`grid place-items-center shrink-0 size-9 rounded-(--r-lg) ${
                    loadedVariant ? 'bg-(--accent-soft) text-(--accent)' : 'bg-(--surface) text-(--muted)'
                  }`}>
                    <ModelIcon size={16} strokeWidth={1.75} />
                  </span>
                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="text-[13px] font-semibold tracking-[-0.01em] text-(--fg) truncate">
                        {opt ? opt.label : 'No model loaded'}
                      </span>
                      {modelDownloading ? (
                        <span className="flex items-center gap-1 text-[10px] font-medium text-(--accent)">
                          <Loader2 size={10} strokeWidth={2} className="animate-spin" />
                          {modelDownloadPct}%
                        </span>
                      ) : null}
                    </div>
                    <div className="mt-0.5 text-[11px] text-(--muted) truncate">
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
          {MODEL_ROWS.map(({ value, Icon, label, trait }) => {
            const opt = MODEL_OPTIONS.find(m => m.value === value)
            return (
              <ModelRow
                key={value}
                icon={<Icon size={15} strokeWidth={1.75} />}
                name={label}
                fullName={opt?.label ?? label}
                trait={trait}
                loaded={loadedVariant === value}
                recommended={recommendedVariant === value}
                installed={onDiskVariants.has(value)}
                downloading={modelDownloading && selected === value}
                downloadPct={modelDownloadPct}
                disabled={modelDownloading || modelSaving}
                onSelect={() => handleModelChange(value)}
                onCancel={cancelDownload}
              />
            )
          })}
        </div>

        {/* Transcription quality */}
        <div className="px-4 py-3.5 border-t border-(--border-soft)">
          <div className="flex items-center justify-between mb-2">
            <div className="text-[13px] tracking-[-0.01em] text-(--fg)">Transcription quality</div>
            <div className="text-[11px] text-(--muted)">Faster is quicker; accurate takes a moment longer.</div>
          </div>
          <div className="flex rounded-(--r-md) border border-(--border) overflow-hidden">
            {([
              { value: 2 as BeamSize, Icon: Zap,      label: 'Fast' },
              { value: 5 as BeamSize, Icon: Scale,    label: 'Balanced' },
              { value: 8 as BeamSize, Icon: Sparkles, label: 'Accurate' },
            ]).map(({ value, Icon, label }, i) => {
              const active = beamSize === value
              return (
                <motion.button
                  key={value}
                  type="button"
                  onClick={() => handleBeamChange(value)}
                  className={`flex-1 flex items-center justify-center gap-1.5 py-2 text-[12px] font-medium cursor-pointer ${
                    i > 0 ? 'border-l border-(--border)' : ''
                  }`}
                  initial={false}
                  animate={{
                    backgroundColor: active ? 'var(--accent-soft)' : 'var(--surface)',
                    color: active ? 'var(--accent)' : 'var(--muted)',
                  }}
                  whileHover={active ? undefined : { backgroundColor: 'var(--surface-hover)', color: 'var(--fg)' }}
                  whileTap={{ scale: 0.97 }}
                  transition={{ type: 'spring', stiffness: 300, damping: 25, mass: 0.8 }}
                >
                  <Icon size={13} strokeWidth={1.75} />
                  {label}
                </motion.button>
              )
            })}
          </div>
        </div>

      </div>

      {/* Smart formatting (local LLM) */}
      <FormattingToggle />

      {/* Microphone */}
      <div className="overflow-hidden rounded-(--r-lg) border border-(--border-soft) bg-(--panel) p-4">
        <MicrophoneSection />
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
