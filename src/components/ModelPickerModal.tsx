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
        <div className="px-7 pt-7 pb-5 border-b border-[var(--border-soft)]">
          <div className="flex items-center gap-3 mb-4">
            <div className="w-9 h-9 rounded-[var(--r-lg)] bg-[var(--accent)] flex items-center justify-center text-[var(--accent-fg)] shadow-[var(--glow)] flex-shrink-0">
              <Zap size={15} strokeWidth={2.5} />
            </div>
            <div>
              <h2 className="text-[16px] font-bold tracking-[-0.025em] text-[var(--fg)] m-0">Choose your AI model</h2>
              <p className="text-[12px] text-[var(--muted)] mt-[2px]">Select once — you can change this later in Settings.</p>
            </div>
          </div>

          {profile && (
            <div className="flex items-center gap-[6px] px-3 py-[6px] rounded-[var(--r-md)] bg-[var(--surface)] border border-[var(--border-soft)] w-fit">
              <Cpu size={11} strokeWidth={1.75} className="text-[var(--muted)]" />
              <span className="text-[11px] text-[var(--fg-2)]">
                {profile.gpuName} · {profile.executionProvider.toUpperCase()}
                {profile.vramGb > 0 ? ` · ${profile.vramGb} GB VRAM` : ''}
              </span>
            </div>
          )}
        </div>

        {/* Model cards */}
        <div className="px-7 py-5 flex flex-col gap-3">
          {MODEL_OPTIONS.map(({ value, label, description, detail, sizeLabel }) => {
            const isRecommended = recommended === value
            const active = selected === value
            return (
              <button
                key={value}
                type="button"
                disabled={confirmed}
                onClick={() => setSelected(value)}
                className={`w-full flex items-start gap-4 px-4 py-4 rounded-[var(--r-lg)] border-[1.5px] text-left cursor-pointer transition-all duration-[var(--t-fast)] disabled:cursor-not-allowed ${
                  active
                    ? 'border-[var(--accent)] bg-[var(--accent-soft)]'
                    : 'border-[var(--border)] bg-[var(--surface)] hover:border-[var(--accent)] hover:bg-[var(--surface-hover)]'
                }`}
              >
                {/* Radio indicator */}
                <div className={`w-[18px] h-[18px] rounded-full border-[1.5px] flex items-center justify-center flex-shrink-0 mt-[1px] transition-all duration-[var(--t-fast)] ${
                  active ? 'border-[var(--accent)] bg-[var(--accent)]' : 'border-[var(--border)]'
                }`}>
                  {active && <Check size={10} strokeWidth={3} className="text-[var(--accent-fg)]" />}
                </div>

                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2 flex-wrap">
                    <span className={`text-[13px] font-semibold leading-none ${active ? 'text-[var(--accent)]' : 'text-[var(--fg)]'}`}>
                      {label}
                    </span>
                    {isRecommended && (
                      <span className="text-[9px] font-bold text-[var(--accent)] bg-[var(--accent-soft)] border border-[color-mix(in_srgb,var(--accent)_30%,transparent)] rounded-[var(--r-xs)] px-[6px] py-[2px] uppercase tracking-[0.05em]">
                        Recommended
                      </span>
                    )}
                  </div>
                  <p className={`text-[11px] mt-[4px] mb-[6px] font-medium ${active ? 'text-[var(--accent)]' : 'text-[var(--fg-2)]'}`}>
                    {description}
                  </p>
                  <p className="text-[11px] text-[var(--muted)] leading-[1.5]">{detail}</p>
                </div>

                <div className="flex items-center gap-[5px] flex-shrink-0 mt-[2px]">
                  <HardDrive size={11} strokeWidth={1.75} className="text-[var(--muted)]" />
                  <span className="text-[11px] text-[var(--muted)] font-medium">{sizeLabel}</span>
                </div>
              </button>
            )
          })}
        </div>

        {/* Footer */}
        <div className="px-7 pb-7 pt-1">
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
