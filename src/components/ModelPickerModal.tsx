import { useState, useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { motion, AnimatePresence } from 'framer-motion'
import { Zap, Check, HardDrive, Cpu } from 'lucide-react'
import { COMMANDS } from '../lib/commands'
import { MODEL_OPTIONS, recommendedToOverride, type ModelOverride } from '../lib/models'
import { useAppStore } from '../store/useAppStore'
import { Button } from '@/components/ui/button'
import type { HardwareProfile, ModelInfo } from '../types'

export function ModelPickerModal() {
  const { setModelChosen, modelDownloading, downloadProgress } = useAppStore()

  const [profile, setProfile] = useState<HardwareProfile | null>(null)
  const [selected, setSelected] = useState<ModelOverride>('medium')
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
      await invoke(COMMANDS.SET_MODEL_OVERRIDE, { variant: selected })
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

  // Close modal once download completes via store events
  useEffect(() => {
    if (confirmed && !modelDownloading && downloadProgress === 100) {
      setModelChosen(true)
    }
  }, [confirmed, modelDownloading, downloadProgress, setModelChosen])

  const recommended = profile ? recommendedToOverride(profile.recommendedModel) : null

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-[2px]">
      <motion.div
        initial={{ opacity: 0, scale: 0.96, y: 8 }}
        animate={{ opacity: 1, scale: 1, y: 0 }}
        transition={{ duration: 0.22, ease: 'easeOut' }}
        className="w-[520px] flex flex-col bg-[var(--panel)] border border-[var(--border)] rounded-[var(--r-xl)] shadow-[var(--shadow-lg)] overflow-hidden"
      >
        {/* Header */}
        <div className="px-6 pt-5 pb-4 border-b border-[var(--border-soft)]">
          <div className="flex items-center gap-3 mb-3">
            <div className="w-8 h-8 rounded-[var(--r-md)] bg-[var(--accent)] flex items-center justify-center text-[var(--accent-fg)] shadow-[var(--glow)] flex-shrink-0">
              <Zap size={14} strokeWidth={2.5} />
            </div>
            <div>
              <h2 className="text-[15px] font-bold tracking-[-0.025em] text-[var(--fg)] m-0">Choose your AI model</h2>
              <p className="text-[11px] text-[var(--muted)] mt-[1px]">Select once — you can change this later in Settings.</p>
            </div>
          </div>

          {profile && (
            <div className="flex items-center gap-[6px] px-[10px] py-[4px] rounded-[var(--r-md)] bg-[var(--surface)] border border-[var(--border-soft)] w-fit">
              <Cpu size={10} strokeWidth={1.75} className="text-[var(--muted)]" />
              <span className="text-[10px] text-[var(--fg-2)]">
                {profile.gpuName} · {profile.executionProvider.toUpperCase()}
                {profile.vramGb > 0 ? ` · ${profile.vramGb} GB VRAM` : ''}
              </span>
            </div>
          )}
        </div>

        {/* Model cards */}
        <div className="px-6 py-4 flex flex-col gap-2">
          {MODEL_OPTIONS.map(({ value, label, description, detail, sizeLabel }) => {
            const isRecommended = recommended === value
            const active = selected === value
            return (
              <motion.button
                key={value}
                type="button"
                disabled={confirmed}
                onClick={() => setSelected(value)}
                className="w-full flex items-start gap-3 px-3 py-3 rounded-[var(--r-lg)] border-[1.5px] text-left cursor-pointer disabled:cursor-not-allowed"
                initial={false}
                animate={{
                  backgroundColor: active ? 'var(--accent-soft)' : 'var(--surface)',
                  borderColor: active ? 'var(--accent)' : 'var(--border)',
                }}
                whileHover={{ backgroundColor: active ? 'var(--accent-soft)' : 'var(--surface-hover)' }}
                whileTap={{ scale: 0.99 }}
                transition={{ type: 'spring', stiffness: 300, damping: 25, mass: 0.8 }}
              >
                {/* Radio indicator */}
                <motion.div
                  className="w-[16px] h-[16px] rounded-full border-[1.5px] flex items-center justify-center flex-shrink-0 mt-[1px]"
                  animate={{
                    borderColor: active ? 'var(--accent)' : 'var(--border)',
                    backgroundColor: active ? 'var(--accent)' : 'transparent',
                  }}
                  transition={{ type: 'spring', stiffness: 300, damping: 25, mass: 0.8 }}
                >
                  {active && <Check size={9} strokeWidth={3} className="text-[var(--accent-fg)]" />}
                </motion.div>

                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2 flex-wrap">
                    <motion.span
                      className="text-[13px] font-semibold leading-none"
                      animate={{ color: active ? 'var(--accent)' : 'var(--fg)' }}
                      transition={{ duration: 0.2 }}
                    >
                      {label}
                    </motion.span>
                    {isRecommended && (
                      <span className="text-[9px] font-bold text-[var(--accent)] bg-[var(--accent-soft)] border border-[color-mix(in_srgb,var(--accent)_30%,transparent)] rounded-[var(--r-xs)] px-[6px] py-[2px] uppercase tracking-[0.05em]">
                        Recommended
                      </span>
                    )}
                  </div>
                  <motion.p
                    className="text-[10px] mt-[2px] mb-[3px] font-medium"
                    animate={{ color: active ? 'var(--accent)' : 'var(--fg-2)' }}
                    transition={{ duration: 0.2 }}
                  >
                    {description}
                  </motion.p>
                  <p className="text-[10px] text-[var(--muted)] leading-[1.4]">{detail}</p>
                </div>

                <div className="flex items-center gap-[5px] flex-shrink-0 mt-[2px]">
                  <HardDrive size={11} strokeWidth={1.75} className="text-[var(--muted)]" />
                  <span className="text-[11px] text-[var(--muted)] font-medium">{sizeLabel}</span>
                </div>
              </motion.button>
            )
          })}
        </div>

        {/* Footer */}
        <div className="px-6 pb-5 pt-1">
          <AnimatePresence mode="wait">
            {confirmed ? (
              <motion.div
                key="downloading"
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                className="flex flex-col gap-2"
              >
                <div className="flex items-center justify-between text-[11px] mb-1">
                  <span className="text-[var(--fg-2)]">
                    {downloadProgress < 100 ? 'Downloading model…' : 'Download complete — loading…'}
                  </span>
                  <span className="text-[var(--accent)] font-semibold tabular-nums">{downloadProgress}%</span>
                </div>
                <div className="h-[3px] rounded-full bg-[var(--border)] overflow-hidden">
                  <motion.div
                    className="h-full rounded-full bg-[var(--accent)]"
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
                  {confirming ? 'Starting download…' : `Download ${MODEL_OPTIONS.find(m => m.value === selected)?.label ?? ''}`}
                </Button>
              </motion.div>
            )}
          </AnimatePresence>
        </div>
      </motion.div>
    </div>
  )
}
