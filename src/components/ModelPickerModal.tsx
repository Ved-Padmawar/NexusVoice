import { useState, useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { motion, AnimatePresence } from 'framer-motion'
import { Zap, HardDrive, Cpu, Bird, Wind, Server, Layers, Box, Gem } from 'lucide-react'
import { COMMANDS } from '../lib/commands'
import {
  WHISPER_PICKER_OPTIONS,
  PARAKEET_PICKER_OPTION,
  PICKER_OPTIONS,
  recommendedToOverride,
  type ModelOverride,
} from '../lib/models'
import { useAppStore } from '../store/useAppStore'
import { Button } from '@/components/ui/button'
import type { HardwareProfile, ModelInfo } from '../types'

/** Selection key: a Whisper override, or the Parakeet engine. */
type Selection = ModelOverride | 'parakeet'

/** Per-tier icons, mirroring the Settings → Models tab. */
const TIER_ICON: Record<ModelOverride, typeof Box> = {
  tiny: Wind,
  base: Server,
  small: Cpu,
  medium: Layers,
  large: Box,
  'large-full': Gem,
}

export function ModelPickerModal() {
  const { setModelChosen, modelDownloading, downloadProgress } = useAppStore()

  const [profile, setProfile] = useState<HardwareProfile | null>(null)
  const [selected, setSelected] = useState<Selection>('medium')
  const [confirming, setConfirming] = useState(false)
  const [confirmed, setConfirmed] = useState(false)

  useEffect(() => {
    // If model already downloaded (e.g. returning user whose modelChosen got reset),
    // skip the modal immediately.
    invoke<ModelInfo>(COMMANDS.GET_MODEL_INFO)
      .then(info => { if (info.downloaded) setModelChosen(true) })
      .catch(() => {})

    invoke<HardwareProfile>(COMMANDS.GET_HARDWARE_PROFILE)
      .then(p => {
        setProfile(p)
        setSelected(recommendedToOverride(p.recommendedModel))
      })
      .catch(() => {})
  }, [setModelChosen])

  const handleConfirm = async () => {
    setConfirming(true)
    try {
      if (selected === 'parakeet') {
        // Parakeet can't be activated until its model is on disk (the backend
        // refuses set_active_engine for a missing model), so download first and
        // switch the active engine on completion (handled in the effect below).
        if (await invoke<boolean>(COMMANDS.DOWNLOAD_PARAKEET).catch(() => false)) {
          setConfirmed(true)
        } else {
          setConfirming(false)
        }
        return
      }

      await invoke(COMMANDS.SET_MODEL_OVERRIDE, { variant: selected })
      // Picking a Whisper model also makes Whisper the active engine.
      await invoke(COMMANDS.SET_ACTIVE_ENGINE, { engine: 'whisper' }).catch(() => {})
      // Check if model is already on disk (user picked same model that was previously downloaded)
      const info = await invoke<ModelInfo>(COMMANDS.GET_MODEL_INFO)
      if (info.downloaded) {
        setModelChosen(true)
        return
      }
      invoke(COMMANDS.RETRY_MODEL_DOWNLOAD).catch(() => {})
      setConfirmed(true)
    } catch {
      setConfirming(false)
    }
  }

  // Close modal once download completes via store events. For Parakeet, switch
  // the active engine now that its model is present before closing.
  useEffect(() => {
    if (confirmed && !modelDownloading && downloadProgress === 100) {
      if (selected === 'parakeet') {
        invoke(COMMANDS.SET_ACTIVE_ENGINE, { engine: 'parakeet' })
          .catch(() => {})
          .finally(() => setModelChosen(true))
      } else {
        setModelChosen(true)
      }
    }
  }, [confirmed, modelDownloading, downloadProgress, selected, setModelChosen])

  // Recommended badge stays on Whisper only — Whisper covers all languages,
  // so it's the safe default. Parakeet is an informed opt-in via its description.
  const recommended = profile ? recommendedToOverride(profile.recommendedModel) : null

  // Overlay starts below the 32px custom title bar (top-8) so the
  // minimize/maximize/close controls + drag region stay clickable. Lighter
  // scrim than a full blur — just enough to focus the modal.
  return (
    <div className="fixed top-8 inset-x-0 bottom-0 z-50 flex items-center justify-center bg-black/15">
      <motion.div
        initial={{ opacity: 0, scale: 0.96, y: 8 }}
        animate={{ opacity: 1, scale: 1, y: 0 }}
        transition={{ duration: 0.22, ease: 'easeOut' }}
        className="w-130 max-h-[90vh] flex flex-col bg-(--panel) border border-(--border) rounded-(--r-xl) shadow-(--shadow-lg) overflow-hidden"
      >
        {/* Header */}
        <div className="shrink-0 px-6 pt-5 pb-4 border-b border-(--border-soft)">
          <div className="flex items-center gap-3 mb-3">
            <div className="w-8 h-8 rounded-(--r-md) bg-(--accent) flex items-center justify-center text-primary-foreground shadow-(--glow) shrink-0">
              <Zap size={14} strokeWidth={2.5} />
            </div>
            <div>
              <h2 className="text-[15px] font-bold tracking-tight text-(--fg) m-0">Choose your AI model</h2>
              <p className="text-[11px] text-muted-foreground mt-px">Select once — you can change this later in Settings.</p>
            </div>
          </div>

          {profile && (
            <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-(--r-md) bg-(--surface) border border-(--border-soft) w-fit">
              <Cpu size={10} strokeWidth={1.75} className="text-muted-foreground" />
              <span className="text-[10px] text-(--fg-2)">
                {profile.gpuName} · {profile.executionProvider.toUpperCase()}
                {profile.vramGb > 0 ? ` · ${profile.vramGb} GB VRAM` : ''}
              </span>
            </div>
          )}
        </div>

        {/* Model cards — sized to fit without scrolling */}
        <div className="flex-1 min-h-0 overflow-y-auto px-6 py-4 flex flex-col gap-2">
          {/* Whisper tiers — the recommended default (broadest language coverage) */}
          <p className="text-[10px] font-bold uppercase tracking-[0.07em] text-muted-foreground mx-0.5 mb-1">
            Whisper — recommended for most users
          </p>
          <div className="grid grid-cols-2 gap-2">
            {WHISPER_PICKER_OPTIONS.map(({ value, label, description, sizeLabel }) => {
              const override = value as ModelOverride
              const isRecommended = recommended === override
              const active = selected === override
              const TierIcon = TIER_ICON[override]
              return (
                <motion.button
                  key={override}
                  type="button"
                  disabled={confirmed}
                  onClick={() => setSelected(override)}
                  className="relative overflow-hidden w-full flex items-start gap-2.5 px-3 py-2.75 rounded-(--r-md) border-[1.5px] text-left cursor-pointer disabled:cursor-not-allowed"
                  initial={false}
                  animate={{
                    backgroundColor: active ? 'var(--accent-soft)' : 'var(--surface)',
                    borderColor: active ? 'var(--accent)' : 'var(--border)',
                  }}
                  whileHover={{ backgroundColor: active ? 'var(--accent-soft)' : 'var(--surface-hover)' }}
                  whileTap={{ scale: 0.99 }}
                  transition={{ type: 'spring', stiffness: 300, damping: 25, mass: 0.8 }}
                >
                  {isRecommended && (
                    <div className="absolute top-0 right-0 bg-(--accent-soft) text-(--accent) border-l border-b border-(--accent) text-[8px] font-extrabold uppercase tracking-[0.05em] px-2 py-[3px] leading-none rounded-tr-(--r-md) rounded-bl-(--r-sm) pointer-events-none">
                      Recommended
                    </div>
                  )}
                  <span className="flex items-center justify-center shrink-0 mt-0.5" style={{ color: active ? 'var(--accent)' : 'var(--fg-2)' }}>
                    <TierIcon size={16} strokeWidth={1.75} />
                  </span>
                  <div className="flex-1 min-w-0">
                    <span className="text-[12.5px] font-semibold leading-none" style={{ color: active ? 'var(--accent)' : 'var(--fg)' }}>
                      {label.replace(/^Whisper /, '')}
                    </span>
                    <p className="text-[10px] text-(--fg-2) leading-[1.3] mt-0.5 line-clamp-2">{description}</p>
                    <span className="flex items-center gap-1 text-[10px] text-muted-foreground mt-1">
                      <HardDrive size={11} strokeWidth={1.75} />
                      {sizeLabel}
                    </span>
                  </div>
                </motion.button>
              )
            })}
          </div>

          {/* Parakeet — alternative fast engine (an option, not the default) */}
          <p className="text-[10px] font-bold uppercase tracking-[0.07em] text-muted-foreground mx-0.5 mt-3 mb-1">
            Alternative — fast, 25 European languages
          </p>
          {(() => {
            const active = selected === 'parakeet'
            return (
              <motion.button
                type="button"
                disabled={confirmed}
                onClick={() => setSelected('parakeet')}
                className="w-full flex items-center gap-3.5 px-4 py-3.5 rounded-(--r-lg) border-[1.5px] text-left cursor-pointer disabled:cursor-not-allowed"
                initial={false}
                animate={{
                  backgroundColor: active ? 'var(--accent-soft)' : 'var(--surface)',
                  borderColor: active ? 'var(--accent)' : 'var(--border)',
                }}
                whileHover={{ backgroundColor: active ? 'var(--accent-soft)' : 'var(--surface-hover)' }}
                whileTap={{ scale: 0.99 }}
                transition={{ type: 'spring', stiffness: 300, damping: 25, mass: 0.8 }}
              >
                <span className="flex items-center justify-center shrink-0" style={{ color: active ? 'var(--accent)' : 'var(--fg-2)' }}>
                  <Bird size={20} strokeWidth={1.75} />
                </span>
                <div className="flex-1 min-w-0">
                  <span className="text-[14px] font-bold leading-none" style={{ color: active ? 'var(--accent)' : 'var(--fg)' }}>
                    {PARAKEET_PICKER_OPTION.label}
                  </span>
                  <p className="text-[11px] text-(--fg-2) leading-[1.35] mt-1">{PARAKEET_PICKER_OPTION.description}</p>
                </div>
                <span className="flex items-center gap-1.25 shrink-0 text-[11px] text-muted-foreground">
                  <HardDrive size={11} strokeWidth={1.75} />
                  {PARAKEET_PICKER_OPTION.sizeLabel}
                </span>
              </motion.button>
            )
          })()}
        </div>

        {/* Footer */}
        <div className="shrink-0 px-6 pb-5 pt-3 border-t border-(--border-soft)">
          <AnimatePresence mode="wait">
            {confirmed ? (
              <motion.div
                key="downloading"
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                className="flex flex-col gap-2"
              >
                <div className="flex items-center justify-between text-[11px] mb-1">
                  <span className="text-(--fg-2)">
                    {downloadProgress < 100 ? 'Downloading model…' : 'Download complete — loading…'}
                  </span>
                  <span className="text-(--accent) font-semibold tabular-nums">{downloadProgress}%</span>
                </div>
                <div className="h-0.75 rounded-full bg-(--border) overflow-hidden">
                  <motion.div
                    className="h-full rounded-full bg-(--accent)"
                    initial={{ width: '0%' }}
                    animate={{ width: `${downloadProgress}%` }}
                    transition={{ duration: 0.3, ease: 'linear' }}
                  />
                </div>
              </motion.div>
            ) : (
              <motion.div key="confirm" initial={{ opacity: 0 }} animate={{ opacity: 1 }}>
                <Button
                  className="w-full"
                  onClick={handleConfirm}
                  disabled={confirming}
                >
                  {confirming ? 'Starting download…' : `Download ${PICKER_OPTIONS.find(m => m.value === selected)?.label ?? ''}`}
                </Button>
              </motion.div>
            )}
          </AnimatePresence>
        </div>
      </motion.div>
    </div>
  )
}
