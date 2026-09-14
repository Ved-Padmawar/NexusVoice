import { useState, useEffect, useMemo } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { motion, AnimatePresence } from 'framer-motion'
import { Check, Cpu, Download, Globe, HardDrive, Loader2, Radio, RefreshCw, ShieldCheck } from 'lucide-react'
import { COMMANDS } from '../lib/commands'
import { extractErrorMessage } from '../lib/errors'
import { formatModelSize, isStreaming, modelNameToId, sortForDisplay, type ModelId } from '../lib/models'
import { vendorForFamily } from '../lib/vendors'
import { useAppStore } from '../store/useAppStore'
import type { HardwareProfile, ModelInfo } from '../types'
import { Button } from './ui/button'
import { Modal, ModalBody, ModalFoot } from './ui/modal'
import { VendorMark } from './ui/VendorMark'

export function ModelPickerModal() {
  const setModelChosen = useAppStore(s => s.setModelChosen)
  const downloads = useAppStore(s => s.downloads)
  const startDownload = useAppStore(s => s.startDownload)
  const refreshModelInfo = useAppStore(s => s.refreshModelInfo)
  const catalog = useAppStore(s => s.catalog)
  const refreshCatalog = useAppStore(s => s.refreshCatalog)

  const [profile, setProfile] = useState<HardwareProfile | null>(null)
  const [selected, setSelected] = useState<ModelId | null>(null)
  const [confirming, setConfirming] = useState(false)
  const [confirmed, setConfirmed] = useState(false)
  // Otherwise Confirm sits disabled with no explanation when a probe fails.
  const [loadError, setLoadError] = useState<string | null>(null)
  const [reloadKey, setReloadKey] = useState(0)

  useEffect(() => {
    // Returning user whose modelChosen got reset but model is on disk: skip the modal.
    invoke<ModelInfo>(COMMANDS.GET_MODEL_INFO)
      .then(info => { if (info.downloaded) setModelChosen(true) })
      .catch(() => {})

    // refreshCatalog swallows its own errors, so an empty catalog is the signal.
    void refreshCatalog().then(() => {
      setLoadError(
        useAppStore.getState().catalog.length === 0 ? 'Could not load the model catalog.' : null,
      )
    })
  }, [setModelChosen, refreshCatalog, reloadKey])

  useEffect(() => {
    if (catalog.length === 0) return
    invoke<HardwareProfile>(COMMANDS.GET_HARDWARE_PROFILE)
      .then(p => {
        setProfile(p)
        setSelected(modelNameToId(p.recommendedModel, catalog))
      })
      .catch((e: unknown) => {
        // A model can still be picked by hand; only the recommendation is lost.
        setLoadError(extractErrorMessage(e, 'Could not detect your hardware'))
      })
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
      void startDownload(selected)
      setConfirmed(true)
    } catch {
      setConfirming(false)
    }
  }

  // The entry is removed when the download finishes, which is the close signal.
  const download = selected ? downloads[selected] : undefined
  useEffect(() => {
    if (confirmed && !download) setModelChosen(true)
  }, [confirmed, download, setModelChosen])

  const recommended = profile ? modelNameToId(profile.recommendedModel, catalog) : null
  const selectedModel = catalog.find(m => m.id === selected) ?? null
  const ordered = useMemo(() => sortForDisplay(catalog), [catalog])

  return (
    <Modal
      title="Choose a speech model"
      description="It runs entirely on this computer, so your audio never leaves it. You can switch any time in Settings."
      icon={<span className="nv-mark nv-mark--accent"><ShieldCheck size={17} strokeWidth={2} /></span>}
      width={700}
    >
      {profile && (
        <div className="px-5.5 pb-3">
          <span className="nv-badge nv-badge--neutral">
            <Cpu />
            {profile.gpuName}, {profile.executionProvider.toUpperCase()}
            {profile.vramGb > 0 ? `, ${profile.vramGb} GB VRAM` : ''}
          </span>
        </div>
      )}

      <ModalBody>
        <div role="radiogroup" aria-label="Speech model" className="grid grid-cols-1 gap-2 sm:grid-cols-2">
          {ordered.map(model => {
            const { id, displayName, description, sizeBytes, multilingual } = model
            const active = selected === id
            const vendor = vendorForFamily(model.family)
            return (
              <button
                key={id}
                type="button"
                role="radio"
                aria-checked={active}
                disabled={confirmed}
                onClick={() => setSelected(id)}
                title={model.detail}
                className="nv-option"
              >
                <span className="nv-radio">{active && <Check strokeWidth={3} />}</span>
                <span className="min-w-0 flex-1">
                  <span className="flex items-center gap-2">
                    {vendor && <VendorMark vendor={vendor} className="size-3.5 shrink-0" />}
                    <span className={`truncate text-[13px] font-semibold ${active ? 'text-accent-text' : 'text-fg'}`}>
                      {displayName}
                    </span>
                    {recommended === id && <span className="nv-badge ml-auto shrink-0">Best fit</span>}
                  </span>
                  <span className="mt-0.5 block truncate text-[12px] text-muted">{description}</span>
                  <span className="mt-1.5 flex items-center gap-3 text-[11.5px] text-muted">
                    <span className="flex items-center gap-1 tabular-nums"><HardDrive size={12} strokeWidth={1.8} />{formatModelSize(sizeBytes)}</span>
                    <span className="flex items-center gap-1"><Globe size={12} strokeWidth={1.8} />{multilingual ? 'Multilingual' : 'English'}</span>
                    {isStreaming(model) && (
                      <span className="flex items-center gap-1 font-semibold text-accent-text"><Radio size={12} strokeWidth={2} />Live</span>
                    )}
                  </span>
                </span>
              </button>
            )
          })}
        </div>
      </ModalBody>

      <ModalFoot>
        <AnimatePresence mode="wait" initial={false}>
          {confirmed ? (
            <motion.div
              key="downloading"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              className="flex w-full items-center gap-3"
            >
              <span className="shrink-0 text-[12.5px] text-fg-2">
                {download ? `Downloading ${selectedModel?.displayName ?? 'model'}…` : 'Download complete, loading…'}
              </span>
              <span className="nv-progress">
                <motion.span
                  className="nv-progress__fill"
                  initial={{ width: '0%' }}
                  animate={{ width: `${download?.progress ?? 100}%` }}
                  transition={{ duration: 0.3, ease: 'linear' }}
                />
              </span>
              <span className="shrink-0 text-[12.5px] font-semibold tabular-nums text-accent-text">
                {download?.progress ?? 100}%
              </span>
            </motion.div>
          ) : (
            <motion.div
              key="confirm"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              className="flex w-full items-center justify-between gap-4"
            >
              <p className={`min-w-0 truncate text-[12.5px] ${loadError ? 'text-danger' : 'text-muted'}`}>
                {loadError ??
                  (selectedModel
                    ? `${selectedModel.displayName}, a ${formatModelSize(selectedModel.sizeBytes)} download`
                    : 'Select a model to continue')}
              </p>
              {loadError && catalog.length === 0 ? (
                <Button className="shrink-0" onClick={() => setReloadKey(k => k + 1)}>
                  <RefreshCw />
                  Try again
                </Button>
              ) : (
                <Button className="shrink-0" onClick={handleConfirm} disabled={confirming || !selected}>
                  {confirming ? <><Loader2 className="nv-spin" />Starting download…</> : <><Download />Download and continue</>}
                </Button>
              )}
            </motion.div>
          )}
        </AnimatePresence>
      </ModalFoot>
    </Modal>
  )
}
