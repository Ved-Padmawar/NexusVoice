import { useState, useEffect, useMemo, useRef } from 'react'
import { createPortal } from 'react-dom'
import { invoke } from '@tauri-apps/api/core'
import { Check, HardDrive, Cpu, Globe, Radio, Download, RefreshCw } from 'lucide-react'
import { COMMANDS } from '../lib/commands'
import { extractErrorMessage } from '../lib/errors'
import { formatModelSize, isStreaming, modelNameToId, sortForDisplay, type ModelId } from '../lib/models'
import { useAppStore } from '../store/useAppStore'
import { VendorMark } from './ui/VendorMark'
import { vendorForFamily } from '../lib/vendors'
import { Spinner } from './Spinner'
import type { HardwareProfile, ModelInfo } from '../types'

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
    <div className="scrim fixed inset-0 z-50 flex items-center justify-center overscroll-none">
      <div className="pop flex max-h-[88vh] w-[min(680px,92vw)] flex-col overflow-hidden">
        <div className="shrink-0 px-6 pb-4 pt-5">
          <h2 className="m-0 text-[16px] font-semibold tracking-[-0.02em] text-(--fg)">
            Choose a transcription model
          </h2>
          <p className="m-0 mt-1 text-[12px] text-(--muted)">
            This runs on your machine, so bigger models need more of it. You can
            change your mind later in Settings.
          </p>

          {profile && (
            <div className="mt-3 flex w-fit items-center gap-2 rounded-(--r-md) bg-(--surface) px-2.5 py-1.5 text-[11px] text-(--fg-2)">
              <Cpu size={11} strokeWidth={1.9} className="text-(--on-soft)" />
              {profile.gpuName} · {profile.executionProvider.toUpperCase()}
              {profile.vramGb > 0 ? ` · ${profile.vramGb} GB VRAM` : ''}
            </div>
          )}
        </div>
        <div className="rule h-px" />

        {/* `overflow` is set from a measurement, not a guess: an always-on
            `auto` box still accepts wheel input when nothing is clipped. */}
        <div
          ref={bodyRef}
          className={`min-h-0 overscroll-contain p-4 ${overflowing ? 'overflow-y-auto' : 'overflow-hidden'}`}
        >
          <div className="grid grid-cols-2 gap-2">
            {ordered.map(model => {
              const { id, displayName, description, sizeBytes, multilingual } = model
              const streaming = isStreaming(model)
              const isRecommended = recommended === id
              const active = selected === id
              const vendor = vendorForFamily(model.family)
              return (
                <button
                  key={id}
                  type="button"
                  disabled={confirmed}
                  onClick={() => setSelected(id)}
                  title={model.detail}
                  aria-pressed={active}
                  className="pick flex items-start gap-2.5 p-3 disabled:cursor-not-allowed"
                >
                  <span className="grid size-7 shrink-0 place-items-center rounded-(--r-sm) bg-(--panel) shadow-[inset_0_0_0_1px_var(--hairline)]">
                    {vendor
                      ? <VendorMark vendor={vendor} className="size-4" />
                      : <Cpu size={13} strokeWidth={1.75} className="text-(--muted)" />}
                  </span>

                  <span className="min-w-0 flex-1">
                    <span className="flex items-center gap-1.5">
                      <span className={`min-w-0 flex-1 truncate text-[12.5px] font-semibold ${active ? 'text-(--on-soft)' : 'text-(--fg)'}`}>
                        {displayName}
                      </span>
                      {isRecommended && (
                        <span className="shrink-0 rounded-(--r-xs) bg-(--accent-soft) px-1.5 py-px text-[9.5px] font-semibold text-(--on-soft)">
                          Best fit
                        </span>
                      )}
                      {active && (
                        <span className="grid size-4 shrink-0 place-items-center rounded-full bg-(--accent)">
                          <Check size={9} strokeWidth={3.5} className="text-(--accent-fg)" />
                        </span>
                      )}
                    </span>
                    <span className="mt-0.5 block truncate text-[11px] text-(--muted)">
                      {description}
                    </span>
                    <span className="mt-1.5 flex items-center gap-2.5 text-[10.5px] text-(--muted)">
                      <span className="flex items-center gap-1 tabular-nums">
                        <HardDrive size={10} strokeWidth={1.9} />
                        {formatModelSize(sizeBytes)}
                      </span>
                      <span className="flex items-center gap-1">
                        <Globe size={10} strokeWidth={1.9} />
                        {multilingual ? 'Multilingual' : 'English'}
                      </span>
                      {streaming && (
                        <span className="flex items-center gap-1 font-medium text-(--on-soft)">
                          <Radio size={10} strokeWidth={2} />
                          Streaming
                        </span>
                      )}
                    </span>
                  </span>
                </button>
              )
            })}
          </div>
        </div>

        <div className="rule h-px" />
        <div className="shrink-0 px-6 py-4">
          {confirmed ? (
            <div className="flex flex-col gap-2">
              <div className="h-1 overflow-hidden rounded-full bg-(--bg-alt)">
                <div
                  className="h-full rounded-full bg-(--accent) transition-[width] duration-300 ease-out"
                  style={{ width: `${download?.progress ?? 100}%` }}
                />
              </div>
              <div className="flex items-baseline justify-between text-[11.5px]">
                <span className="text-(--fg-2)">
                  {download ? 'Downloading model' : 'Download complete, loading'}
                </span>
                <span className="font-semibold tabular-nums text-(--on-soft)">
                  {download?.progress ?? 100}%
                </span>
              </div>
            </div>
          ) : (
            <div className="flex items-center justify-between gap-4">
              <p className={`m-0 min-w-0 truncate text-[11.5px] ${loadError ? 'text-(--danger)' : 'text-(--muted)'}`}>
                {loadError ??
                  (selectedModel
                    ? `${selectedModel.displayName} · ${formatModelSize(selectedModel.sizeBytes)} download`
                    : 'Select a model to continue')}
              </p>
              {loadError && catalog.length === 0 ? (
                <button type="button" onClick={() => setReloadKey(k => k + 1)} className="btn btn-primary min-w-44">
                  <RefreshCw size={13} strokeWidth={2} />
                  Try again
                </button>
              ) : (
                <button
                  type="button"
                  onClick={handleConfirm}
                  disabled={confirming || !selected}
                  className="btn btn-primary min-w-44"
                >
                  {confirming ? (
                    <>
                      <Spinner size={13} />
                      Starting download
                    </>
                  ) : (
                    <>
                      <Download size={13} strokeWidth={2} />
                      Download and continue
                    </>
                  )}
                </button>
              )}
            </div>
          )}
        </div>
      </div>
    </div>,
    document.body
  )
}
