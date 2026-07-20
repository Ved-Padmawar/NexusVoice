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
  const setModelChosen = useAppStore(s => s.setModelChosen)
  const modelDownloading = useAppStore(s => s.modelDownloading)
  const downloadProgress = useAppStore(s => s.downloadProgress)
  const refreshModelInfo = useAppStore(s => s.refreshModelInfo)

  const [profile, setProfile] = useState<HardwareProfile | null>(null)
  const [selected, setSelected] = useState<ModelOverride>('medium')
  const [confirming, setConfirming] = useState(false)
  const [confirmed, setConfirmed] = useState(false)

  useEffect(() => {
    // Returning user whose modelChosen got reset but model is on disk: skip the modal.
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
      await refreshModelInfo()
      // Already on disk (picked a previously-downloaded model): skip the download step.
      if (useAppStore.getState().activeModelDownloaded) {
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
        className="w-130 flex flex-col bg-(--panel) border border-(--border) rounded-(--r-xl) shadow-(--shadow-lg) overflow-hidden"
      >
        {/* Header */}
        <div className="px-6 pt-5 pb-4 border-b border-(--border-soft)">
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
                className="w-full flex items-start gap-3 px-3 py-3 rounded-(--r-lg) border-[1.5px] text-left cursor-pointer disabled:cursor-not-allowed"
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
                  className="w-4 h-4 rounded-full border-[1.5px] flex items-center justify-center shrink-0 mt-px"
                  animate={{
                    borderColor: active ? 'var(--accent)' : 'var(--border)',
                    backgroundColor: active ? 'var(--accent)' : 'transparent',
                  }}
                  transition={{ type: 'spring', stiffness: 300, damping: 25, mass: 0.8 }}
                >
                  {active && <Check size={9} strokeWidth={3} className="text-primary-foreground" />}
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
                      <span className="text-[9px] font-bold text-(--accent) bg-(--accent-soft) border border-[color-mix(in_srgb,var(--accent)_30%,transparent)] rounded-(--r-xs) px-1.5 py-0.5 uppercase tracking-wider">
                        Recommended
                      </span>
                    )}
                  </div>
                  <motion.p
                    className="text-[10px] mt-0.5 mb-0.75 font-medium"
                    animate={{ color: active ? 'var(--accent)' : 'var(--fg-2)' }}
                    transition={{ duration: 0.2 }}
                  >
                    {description}
                  </motion.p>
                  <p className="text-[10px] text-muted-foreground leading-[1.4]">{detail}</p>
                </div>

                <div className="flex items-center gap-1.25 shrink-0 mt-0.5">
                  <HardDrive size={11} strokeWidth={1.75} className="text-muted-foreground" />
                  <span className="text-[11px] text-muted-foreground font-medium">{sizeLabel}</span>
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
