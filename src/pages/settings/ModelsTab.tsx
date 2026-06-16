import { useState, useCallback, useEffect, useRef } from 'react'
import { motion } from 'framer-motion'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { toast } from 'sonner'
import {
  Bird, Wind, Server, Cpu, Layers, Box, Gem,
  HardDrive, Check, Trash2, Download, Database,
  Zap, Scale, Sparkles,
} from 'lucide-react'
import { COMMANDS } from '../../lib/commands'
import { EVENTS } from '../../lib/events'
import { detectPlatform } from '../../lib/platform'
import { recommendedToOverride, type ModelOverride, type Engine } from '../../lib/models'
import type { HardwareProfile } from '../../types'
import { useAppStore } from '../../store/useAppStore'
import type { BeamSize } from '../../store/uiSlice'

const PARAKEET_SUPPORTED = detectPlatform() === 'windows'

type Card = {
  id: string
  engine: Engine
  /** Whisper override key; undefined for Parakeet. */
  override?: ModelOverride
  Icon: typeof Box
  name: string
  /** Real model identifier shown in the active-model header badge. */
  file: string
  size: string
  desc: string
}

const PARAKEET_CARD: Card = {
  id: 'parakeet', engine: 'parakeet', Icon: Bird, name: 'Parakeet', file: 'Parakeet v3',
  size: '2.4 GB', desc: 'Fast and accurate. Multilingual, auto-detected.',
}

const WHISPER_CARDS: Card[] = [
  { id: 'tiny',       engine: 'whisper', override: 'tiny',       Icon: Wind,   name: 'Tiny',   file: 'Whisper Tiny',          size: '75 MB',  desc: 'Fastest, lowest accuracy. English only.' },
  { id: 'base',       engine: 'whisper', override: 'base',       Icon: Server, name: 'Base',   file: 'Whisper Base',          size: '142 MB', desc: 'Fast, basic accuracy for low-end hardware. English only.' },
  { id: 'small',      engine: 'whisper', override: 'small',      Icon: Cpu,    name: 'Small',  file: 'Whisper Small',         size: '466 MB', desc: 'Standard accuracy, light footprint. English only.' },
  { id: 'medium',     engine: 'whisper', override: 'medium',     Icon: Layers, name: 'Medium', file: 'Whisper Medium',        size: '1.5 GB', desc: 'Balanced performance and accuracy. English only.' },
  { id: 'large',      engine: 'whisper', override: 'large',      Icon: Box,    name: 'Turbo',  file: 'Whisper Large v3 Turbo', size: '1.6 GB', desc: 'High accuracy and fast. Multilingual, all languages.' },
  { id: 'large-full', engine: 'whisper', override: 'large-full', Icon: Gem,    name: 'Max',    file: 'Whisper Large v3',      size: '3.1 GB', desc: 'Maximum accuracy, heaviest. Multilingual.' },
]

const CARDS: Card[] = PARAKEET_SUPPORTED ? [PARAKEET_CARD, ...WHISPER_CARDS] : WHISPER_CARDS

type DownloadedModel = { variant: string; sizeBytes: number; isActive: boolean }

const BEAMS: { value: BeamSize; Icon: typeof Box; label: string; desc: string }[] = [
  { value: 2, Icon: Zap,      label: 'Fast',     desc: 'Lower latency' },
  { value: 5, Icon: Scale,    label: 'Balanced', desc: 'Recommended' },
  { value: 8, Icon: Sparkles, label: 'Accurate', desc: 'Best quality' },
]

function formatBytes(bytes: number): string {
  if (bytes <= 0) return '0 MB'
  const gb = bytes / 1e9
  if (gb >= 1) return `${gb.toFixed(2)} GB`
  return `${Math.round(bytes / 1e6)} MB`
}

function nameToOverride(name: string): ModelOverride {
  const n = name.toLowerCase()
  return n.includes('large') && !n.includes('turbo') ? 'large-full'
    : n.includes('large') ? 'large'
      : n.includes('medium') ? 'medium'
        : n.includes('small') ? 'small'
          : n.includes('base') ? 'base' : 'tiny'
}

export function ModelsTab() {
  const [profile, setProfile] = useState<HardwareProfile | null>(null)
  const [engine, setEngine] = useState<Engine>('whisper')
  const [activeName, setActiveName] = useState('Whisper Large v3 Turbo')
  const [downloaded, setDownloaded] = useState<Record<string, number>>({})
  const [busy, setBusy] = useState(false)

  const beamSize = useAppStore(s => s.beamSize)
  const setBeamSize = useAppStore(s => s.setBeamSize)

  const refresh = useCallback(async () => {
    const [eng, info, models] = await Promise.all([
      invoke<string>(COMMANDS.GET_ACTIVE_ENGINE).catch(() => 'whisper'),
      invoke<{ modelName: string }>(COMMANDS.GET_MODEL_INFO).catch(() => ({ modelName: '' })),
      invoke<DownloadedModel[]>(COMMANDS.GET_DOWNLOADED_MODELS).catch(() => []),
    ])
    if (eng === 'whisper' || eng === 'parakeet') setEngine(eng)
    if (info.modelName) setActiveName(info.modelName)
    const map: Record<string, number> = {}
    for (const m of models) map[m.variant] = m.sizeBytes
    setDownloaded(map)
  }, [])

  useEffect(() => {
    let cancelled = false
    void (async () => {
      const [hw, beam] = await Promise.all([
        invoke<HardwareProfile>(COMMANDS.GET_HARDWARE_PROFILE).catch(() => null),
        invoke<number>(COMMANDS.GET_BEAM_SIZE).catch(() => 5),
      ])
      if (cancelled) return
      if (hw) setProfile(hw)
      setBeamSize((beam === 2 || beam === 5 || beam === 8) ? beam as BeamSize : 5)
      await refresh()
    })()
    return () => { cancelled = true }
  }, [refresh, setBeamSize])

  // Which Whisper tier is active, derived from the active model name.
  const activeOverride = nameToOverride(activeName)

  const isActive = (c: Card) =>
    engine === c.engine && (c.engine === 'parakeet' || c.override === activeOverride)

  const isDownloaded = (c: Card) => c.id in downloaded

  // ---- Actions ----
  const downloadWatchers = useRef<Array<() => void>>([])
  const stopWatching = () => { downloadWatchers.current.forEach(fn => fn()); downloadWatchers.current = [] }
  useEffect(() => stopWatching, [])

  const pickCard = async (c: Card) => {
    if (busy || isActive(c)) return
    if (c.engine === 'whisper') {
      setBusy(true)
      try {
        await invoke(COMMANDS.SET_ACTIVE_ENGINE, { engine: 'whisper' })
        setEngine('whisper')
        await invoke(COMMANDS.SET_MODEL_OVERRIDE, { variant: c.override })
        if (!isDownloaded(c)) invoke(COMMANDS.RETRY_MODEL_DOWNLOAD).catch(() => {})
        await refresh()
        toast.success(`Switched to ${c.name}`)
      } catch { /* ignore */ }
      finally { setBusy(false) }
      return
    }
    // Parakeet: switch immediately if present, else download then switch.
    setBusy(true)
    if (isDownloaded(c)) {
      try {
        await invoke(COMMANDS.SET_ACTIVE_ENGINE, { engine: 'parakeet' })
        setEngine('parakeet')
        await refresh()
        toast.success('Switched to Parakeet')
      } catch { /* ignore */ }
      finally { setBusy(false) }
      return
    }
    startParakeetDownload()
  }

  const startParakeetDownload = () => {
    stopWatching()
    const done = async () => {
      stopWatching()
      try {
        await invoke(COMMANDS.SET_ACTIVE_ENGINE, { engine: 'parakeet' })
        setEngine('parakeet')
        await refresh()
        toast.success('Switched to Parakeet')
      } catch { toast.error('Could not switch to Parakeet') }
      finally { setBusy(false) }
    }
    const fail = () => { stopWatching(); toast.error('Parakeet download failed'); setBusy(false) }
    listen(EVENTS.MODEL_DOWNLOAD_COMPLETE, done).then(u => downloadWatchers.current.push(u))
    listen(EVENTS.MODEL_DOWNLOAD_ERROR, fail).then(u => downloadWatchers.current.push(u))
    invoke(COMMANDS.DOWNLOAD_PARAKEET).catch(() => fail())
  }

  const downloadCard = (c: Card) => {
    if (busy) return
    if (c.engine === 'parakeet') { setBusy(true); startParakeetDownload(); return }
    setBusy(true)
    invoke(COMMANDS.SET_MODEL_OVERRIDE, { variant: c.override })
      .then(() => invoke(COMMANDS.RETRY_MODEL_DOWNLOAD))
      .then(() => refresh())
      .catch(() => {})
      .finally(() => setBusy(false))
  }

  const deleteCard = async (c: Card) => {
    if (busy || isActive(c)) return
    setBusy(true)
    try {
      await invoke(COMMANDS.DELETE_MODEL, { variant: c.id })
      await refresh()
      toast.success(`Deleted ${c.name}`)
    } catch { toast.error('Could not delete model') }
    finally { setBusy(false) }
  }

  const changeBeam = (v: BeamSize) => {
    setBeamSize(v)
    invoke(COMMANDS.SET_BEAM_SIZE, { beamSize: v }).catch(() => {})
  }

  const recommendedOverride = profile ? recommendedToOverride(profile.recommendedModel) : null
  const downloadedCount = Object.keys(downloaded).length
  const totalBytes = Object.values(downloaded).reduce((a, b) => a + b, 0)

  return (
    <div className="flex flex-col gap-4">
      {/* Section header: title + active-model badge + disk badge */}
      <div className="flex items-start justify-between gap-4">
        <div>
          <p className="text-[12px] font-semibold text-[var(--fg-2)] tracking-[-0.01em]">Speech-to-text model</p>
          <p className="text-[11px] text-[var(--muted)] mt-[3px] flex items-center gap-1">
            <Cpu size={10} strokeWidth={1.75} />
            {profile
              ? `${profile.gpuName} · ${profile.executionProvider.toUpperCase()}${profile.vramGb > 0 ? ` · ${profile.vramGb}GB VRAM` : ''} · selecting a model also switches its engine`
              : 'Detecting hardware…'}
          </p>
        </div>
        <div className="flex items-center gap-2 flex-shrink-0">
          <span className="flex items-center gap-1.5 text-[11px] font-semibold text-[var(--accent)] bg-[var(--accent-soft)] border border-[var(--accent)] px-2.5 py-[5px] rounded-[var(--r-md)]">
            {activeName}
          </span>
          <span className="flex items-center gap-1.5 text-[11px] font-semibold text-[var(--fg-2)] bg-[var(--surface)] border border-[var(--border-soft)] px-2.5 py-[5px] rounded-[var(--r-md)]">
            <Database size={12} strokeWidth={1.75} className="text-[var(--fg-2)]" />
            <b className="text-[var(--accent)] font-bold">{downloadedCount}</b> of {CARDS.length} · {formatBytes(totalBytes)}
          </span>
        </div>
      </div>

      {/* Unified card grid */}
      <div className="grid gap-2.5" style={{ gridTemplateColumns: 'repeat(4, minmax(0, 1fr))' }}>
        {CARDS.map((c) => {
          const active = isActive(c)
          const dl = isDownloaded(c)
          const recommended = c.engine === 'whisper' && c.override === recommendedOverride
          return (
            <motion.div
              key={c.id}
              role="button"
              tabIndex={0}
              className="relative overflow-hidden flex flex-col gap-2 min-w-0 p-3.5 rounded-[var(--r-lg)] border-[1.5px] cursor-pointer"
              initial={false}
              animate={{
                backgroundColor: active ? 'var(--accent-soft)' : 'var(--surface)',
                borderColor: active ? 'var(--accent)' : 'var(--border)',
              }}
              whileHover={{ backgroundColor: active ? 'var(--accent-soft)' : 'var(--surface-hover)', y: -1 }}
              transition={{ type: 'spring', stiffness: 300, damping: 25, mass: 0.8 }}
              onClick={() => pickCard(c)}
              onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); pickCard(c) } }}
            >
              {recommended && (
                <div className="absolute top-0 right-0 bg-[var(--accent-soft)] text-[var(--accent)] border-l border-b border-[var(--accent)] text-[8px] font-extrabold uppercase tracking-[0.05em] px-2 py-[3px] leading-none rounded-tr-[var(--r-lg)] rounded-bl-[var(--r-md)] pointer-events-none">
                  Recommended
                </div>
              )}
              <div className="flex items-center gap-2.5 min-w-0">
                <span className="flex items-center justify-center flex-shrink-0" style={{ color: active ? 'var(--accent)' : 'var(--fg-2)' }}>
                  <c.Icon size={18} strokeWidth={1.75} />
                </span>
                <span className="text-[14px] font-bold tracking-[-0.01em] min-w-0 overflow-hidden text-ellipsis whitespace-nowrap" style={{ color: active ? 'var(--accent)' : 'var(--fg)' }}>
                  {c.name}
                </span>
              </div>
              <div className="text-[11px] text-[var(--fg-2)] leading-[1.45] min-h-[46px]">
                {c.desc}
              </div>
              <div className="flex items-center justify-between gap-2 mt-auto pt-2.5 border-t border-[var(--border-soft)]">
                <span className="inline-flex items-center gap-1.5 text-[10.5px] font-semibold text-[var(--muted)] whitespace-nowrap">
                  <HardDrive size={12} strokeWidth={1.75} className="flex-shrink-0" />
                  {c.size}
                </span>
                {dl ? (
                  <div className="flex items-center gap-1.5">
                    <span className="inline-flex items-center gap-1.5 text-[10.5px] font-semibold text-[var(--success)]">
                      <Check size={13} strokeWidth={2.5} />
                      Downloaded
                    </span>
                    <button
                      type="button"
                      className="w-[26px] h-[26px] rounded-[var(--r-md)] flex items-center justify-center bg-transparent border border-[var(--border)] text-[var(--muted)] cursor-pointer disabled:opacity-30 disabled:cursor-not-allowed enabled:hover:bg-[var(--danger)] enabled:hover:border-[var(--danger)] enabled:hover:text-white"
                      disabled={active}
                      title={active ? "Active model — can't be deleted" : 'Delete model'}
                      onClick={(e) => { e.stopPropagation(); deleteCard(c) }}
                    >
                      <Trash2 size={13} strokeWidth={1.75} />
                    </button>
                  </div>
                ) : (
                  <button
                    type="button"
                    className="inline-flex items-center gap-1.5 px-2.5 h-[26px] rounded-[var(--r-md)] bg-[var(--accent)] text-[var(--accent-fg)] border-none text-[10.5px] font-semibold cursor-pointer hover:bg-[var(--accent-hover)]"
                    onClick={(e) => { e.stopPropagation(); downloadCard(c) }}
                  >
                    <Download size={12} strokeWidth={2} />
                    Download
                  </button>
                )}
              </div>
            </motion.div>
          )
        })}
      </div>

      {/* Transcription quality — Whisper only (Parakeet/TDT has no beam search) */}
      {engine === 'whisper' && (
        <div className="flex flex-col gap-3 pt-2 border-t border-[var(--border-soft)]">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-[12px] font-semibold text-[var(--fg-2)] tracking-[-0.01em]">Transcription quality</p>
              <p className="text-[11px] text-[var(--muted)] mt-[3px]">Beam search width — Whisper only. Faster is quicker; Accurate takes a moment longer.</p>
            </div>
            <span className="text-[10px] font-mono font-semibold text-[var(--accent)] bg-[var(--accent-soft)] border border-[var(--accent-soft)] px-[6px] py-px rounded-[var(--r-sm)]">
              beam · {beamSize}
            </span>
          </div>
          <div className="flex gap-2.5">
            {BEAMS.map(({ value, Icon, label, desc }) => {
              const active = beamSize === value
              return (
                <motion.button
                  key={value}
                  type="button"
                  className="flex-1 flex flex-col gap-[3px] px-3.5 py-[9px] rounded-[var(--r-lg)] border-[1.5px] cursor-pointer text-left"
                  initial={false}
                  animate={{
                    backgroundColor: active ? 'var(--accent-soft)' : 'var(--surface)',
                    borderColor: active ? 'var(--accent)' : 'var(--border)',
                  }}
                  whileHover={{ backgroundColor: active ? 'var(--accent-soft)' : 'var(--surface-hover)' }}
                  whileTap={{ scale: 0.98 }}
                  transition={{ type: 'spring', stiffness: 300, damping: 25, mass: 0.8 }}
                  onClick={() => changeBeam(value)}
                >
                  <div className="flex items-center gap-2">
                    <motion.span animate={{ color: active ? 'var(--accent)' : 'var(--muted)' }} transition={{ duration: 0.2 }} className="flex">
                      <Icon size={14} strokeWidth={1.75} />
                    </motion.span>
                    <motion.span className="text-[12.5px] font-bold" animate={{ color: active ? 'var(--accent)' : 'var(--fg)' }} transition={{ duration: 0.2 }}>
                      {label}
                    </motion.span>
                  </div>
                  <motion.span className="text-[10.5px]" animate={{ color: active ? 'var(--accent)' : 'var(--muted)' }} transition={{ duration: 0.2 }}>
                    {desc}
                  </motion.span>
                </motion.button>
              )
            })}
          </div>
        </div>
      )}
    </div>
  )
}
