import { useState, useEffect, useMemo, useRef } from 'react'
import { createPortal } from 'react-dom'
import { invoke } from '@tauri-apps/api/core'
import { motion, AnimatePresence } from 'framer-motion'
import { Zap, Check, HardDrive, Cpu, Globe, Radio, Download, Loader2 } from 'lucide-react'
import { COMMANDS } from '../lib/commands'
import { formatModelSize, isStreaming, modelNameToId, sortForDisplay, type ModelId } from '../lib/models'
import { useAppStore } from '../store/useAppStore'
import { Button } from '@/components/ui/button'
import type { HardwareProfile, ModelInfo } from '../types'

export function ModelPickerModal() {
  const setModelChosen = useAppStore(s => s.setModelChosen)
  const modelDownloading = useAppStore(s => s.modelDownloading)
  const downloadProgress = useAppStore(s => s.downloadProgress)
  const refreshModelInfo = useAppStore(s => s.refreshModelInfo)
  const catalog = useAppStore(s => s.catalog)
  const refreshCatalog = useAppStore(s => s.refreshCatalog)

  const [profile, setProfile] = useState<HardwareProfile | null>(null)
  const [selected, setSelected] = useState<ModelId | null>(null)
  const [confirming, setConfirming] = useState(false)
  const [confirmed, setConfirmed] = useState(false)

  useEffect(() => {
    // Returning user whose modelChosen got reset but model is on disk: skip the modal.
    invoke<ModelInfo>(COMMANDS.GET_MODEL_INFO)
      .then(info => { if (info.downloaded) setModelChosen(true) })
      .catch(() => {})

    void refreshCatalog()
  }, [setModelChosen, refreshCatalog])

  useEffect(() => {
    if (catalog.length === 0) return
    invoke<HardwareProfile>(COMMANDS.GET_HARDWARE_PROFILE)
      .then(p => {
        setProfile(p)
        setSelected(modelNameToId(p.recommendedModel, catalog))
      })
      .catch(() => {})
  }, [catalog])

  const handleConfirm = async () => {
    if (!selected) return
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

  const recommended = profile ? modelNameToId(profile.recommendedModel, catalog) : null
  const selectedModel = catalog.find(m => m.id === selected) ?? null
  const ordered = useMemo(() => sortForDisplay(catalog), [catalog])

  // Enable scrolling only when the grid is actually clipped.
  const bodyRef = useRef<HTMLDivElement>(null)
  const [overflowing, setOverflowing] = useState(false)
  useEffect(() => {
    const el = bodyRef.current
    if (!el) return
    const measure = () => setOverflowing(el.scrollHeight > el.clientHeight + 1)
    measure()
    const observer = new ResizeObserver(measure)
    observer.observe(el)
    window.addEventListener('resize', measure)
    return () => {
      observer.disconnect()
      window.removeEventListener('resize', measure)
    }
  }, [ordered.length])

  // Portaled out of #root, which is a fixed-height `overflow: hidden` box.
  return createPortal(
    <div className="fixed inset-0 z-50 flex items-center justify-center overscroll-none bg-black/60 backdrop-blur-[2px]">
      <motion.div
        initial={{ opacity: 0, scale: 0.96, y: 8 }}
        animate={{ opacity: 1, scale: 1, y: 0 }}
        transition={{ duration: 0.22, ease: 'easeOut' }}
        className="w-[min(660px,92vw)] max-h-[88vh] flex flex-col bg-(--panel) border border-(--border) rounded-(--r-xl) shadow-(--shadow-lg) overflow-hidden"
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

        {/* `overflow` is set from a measurement, not a guess: an always-on
            `auto` box still accepts wheel input when nothing is clipped. */}
        <div
          ref={bodyRef}
          className={`min-h-0 overscroll-contain px-6 py-4 ${
            overflowing ? 'overflow-y-auto' : 'overflow-hidden'
          }`}
        >
          <div className="grid grid-cols-2 gap-2">
            {ordered.map(model => {
              const { id, displayName, description, sizeBytes, multilingual } = model
              const streaming = isStreaming(model)
              const isRecommended = recommended === id
              const active = selected === id
              return (
                <motion.button
                  key={id}
                  type="button"
                  disabled={confirmed}
                  onClick={() => setSelected(id)}
                  title={model.detail}
                  className="flex items-start gap-2.5 px-3 py-2 rounded-(--r-lg) border-[1.5px] text-left cursor-pointer disabled:cursor-not-allowed"
                  initial={false}
                  animate={{
                    backgroundColor: active ? 'var(--accent-soft)' : 'var(--surface)',
                    borderColor: active ? 'var(--accent)' : 'var(--border)',
                  }}
                  whileHover={{ backgroundColor: active ? 'var(--accent-soft)' : 'var(--surface-hover)' }}
                  whileTap={{ scale: 0.99 }}
                  transition={{ type: 'spring', stiffness: 300, damping: 25, mass: 0.8 }}
                >
                  <motion.div
                    className="w-4 h-4 rounded-full border-[1.5px] flex items-center justify-center shrink-0 mt-0.5"
                    animate={{
                      borderColor: active ? 'var(--accent)' : 'var(--border)',
                      backgroundColor: active ? 'var(--accent)' : 'transparent',
                    }}
                    transition={{ type: 'spring', stiffness: 300, damping: 25, mass: 0.8 }}
                  >
                    {active && <Check size={9} strokeWidth={3} className="text-primary-foreground" />}
                  </motion.div>

                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-1.5">
                      <motion.span
                        className="text-[12.5px] font-semibold leading-tight truncate"
                        animate={{ color: active ? 'var(--accent)' : 'var(--fg)' }}
                        transition={{ duration: 0.2 }}
                      >
                        {displayName}
                      </motion.span>
                      {isRecommended && (
                        <span className="shrink-0 text-[8.5px] font-bold text-(--accent) bg-(--accent-soft) border border-[color-mix(in_srgb,var(--accent)_30%,transparent)] rounded-(--r-xs) px-1 py-px uppercase tracking-wider">
                          Best
                        </span>
                      )}
                    </div>
                    <p className="text-[10.5px] text-muted-foreground leading-snug truncate">
                      {description}
                    </p>
                    <div className="flex items-center gap-2 mt-1 text-[10px] text-muted-foreground">
                      <span className="flex items-center gap-1">
                        <HardDrive size={10} strokeWidth={1.75} />
                        <span className="tabular-nums">{formatModelSize(sizeBytes)}</span>
                      </span>
                      <span className="flex items-center gap-1">
                        <Globe size={10} strokeWidth={1.75} />
                        {multilingual ? 'Multilingual' : 'English'}
                      </span>
                      {streaming && (
                        <span className="flex items-center gap-1 text-(--accent) font-semibold">
                          <Radio size={10} strokeWidth={2} />
                          Live
                        </span>
                      )}
                    </div>
                  </div>
                </motion.button>
              )
            })}
          </div>
        </div>

        {/* Footer */}
        <div className="shrink-0 px-6 pb-5 pt-4 border-t border-(--border-soft)">
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
              <motion.div
                key="confirm"
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                className="flex items-center justify-between gap-4"
              >
                <p className="text-[11px] text-muted-foreground m-0 min-w-0 truncate">
                  {selectedModel
                    ? `${selectedModel.displayName} · ${formatModelSize(selectedModel.sizeBytes)} download`
                    : 'Select a model to continue'}
                </p>
                <Button
                  className="shrink-0 min-w-44 gap-2"
                  onClick={handleConfirm}
                  disabled={confirming || !selected}
                >
                  {confirming ? (
                    <>
                      <Loader2 size={14} strokeWidth={2} className="animate-spin" />
                      Starting download…
                    </>
                  ) : (
                    <>
                      <Download size={14} strokeWidth={2} />
                      Download &amp; continue
                    </>
                  )}
                </Button>
              </motion.div>
            )}
          </AnimatePresence>
        </div>
      </motion.div>
    </div>,
    document.body
  )
}
