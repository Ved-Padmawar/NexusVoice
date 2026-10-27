import { useState, useEffect, useMemo, useRef } from 'react'
import { createPortal } from 'react-dom'
import { LightningIcon, CheckIcon, HardDrivesIcon, GraphicsCardIcon, GlobeIcon, BroadcastIcon, DownloadSimpleIcon, CircleNotchIcon, ArrowsClockwiseIcon } from '@phosphor-icons/react'
import { commands } from '../bindings'
import { extractErrorMessage } from '../lib/errors'
import { cn } from '../lib/utils'
import { formatModelSize, isStreaming, modelNameToId, sortForDisplay, type ModelId } from '../lib/models'
import { useAppStore } from '../store/useAppStore'
import { Button } from './ui/Button'
import type { HardwareProfile } from '../types'

export function ModelPickerModal() {
  const setModelChosen = useAppStore(s => s.setModelChosen)
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
    commands.getModelInfo()
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
    commands.getHardwareProfile()
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
      await commands.setModelOverride(selected)
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
  const download = useAppStore(s => (selected ? s.downloads[selected] : undefined))
  useEffect(() => {
    if (confirmed && !download) setModelChosen(true)
  }, [confirmed, download, setModelChosen])

  const recommended = profile ? modelNameToId(profile.recommendedModel, catalog) : null
  const selectedModel = catalog.find(m => m.id === selected) ?? null
  const ordered = useMemo(() => sortForDisplay(catalog), [catalog])

  // Scroll only when a card is clipped, not just the bottom padding.
  const bodyRef = useRef<HTMLDivElement>(null)
  const gridRef = useRef<HTMLDivElement>(null)
  const [overflowing, setOverflowing] = useState(false)
  useEffect(() => {
    const el = bodyRef.current
    const grid = gridRef.current
    if (!el || !grid) return
    const measure = () => {
      const top = parseFloat(getComputedStyle(el).paddingTop)
      setOverflowing(top + grid.offsetHeight > el.clientHeight)
    }
    measure()
    const observer = new ResizeObserver(measure)
    observer.observe(el)
    observer.observe(grid)
    window.addEventListener('resize', measure)
    return () => {
      observer.disconnect()
      window.removeEventListener('resize', measure)
    }
  }, [ordered.length])

  // Portaled out of #root, which is a fixed-height `overflow: hidden` box.
  return createPortal(
    <div className="fixed inset-0 z-50 flex items-center justify-center overscroll-none bg-black/60 backdrop-blur-[2px]">
      <div className="w-[min(660px,92vw)] max-h-[88vh] flex flex-col bg-panel border border-line rounded-xl shadow-(--shadow-lg) overflow-hidden transition-[opacity,scale,translate] duration-220 ease-out starting:translate-y-2 starting:scale-96 starting:opacity-0">
        <div className="shrink-0 px-6 pt-5 pb-4 border-b border-line-soft">
          <div className="flex items-center gap-3 mb-3">
            <div className="w-8 h-8 rounded-md bg-accent flex items-center justify-center text-accent-fg shrink-0">
              <LightningIcon size={14} weight="bold" />
            </div>
            <div>
              <h2 className="text-[15px] font-bold tracking-tight text-fg">Choose your AI model</h2>
              <p className="text-[11px] text-muted mt-px">Select once — you can change this later in Settings.</p>
            </div>
          </div>

          {profile && (
            <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-surface border border-line-soft w-fit">
              <GraphicsCardIcon size={10} className="text-muted" />
              <span className="text-[10px] text-fg-2">
                {profile.gpuName} · {profile.executionProvider.toUpperCase()}
                {profile.vramGb > 0 ? ` · ${profile.vramGb} GB VRAM` : ''}
              </span>
            </div>
          )}
        </div>

        {/* Measured: an always-on `auto` box still takes wheel input when nothing is clipped. */}
        <div
          ref={bodyRef}
          className={`min-h-0 overscroll-none overflow-x-hidden px-6 pt-2.75 pb-4 ${
            overflowing ? 'overflow-y-auto' : 'overflow-y-hidden'
          }`}
        >
          <div ref={gridRef} className="grid grid-cols-2 gap-2">
            {ordered.map(model => {
              const { id, displayName, description, sizeBytes, multilingual } = model
              const streaming = isStreaming(model)
              const isRecommended = recommended === id
              const active = selected === id
              return (
                <button
                  key={id}
                  type="button"
                  disabled={confirmed}
                  onClick={() => setSelected(id)}
                  title={model.detail}
                  className={cn(
                    'flex items-start gap-2.5 px-3 py-2 rounded-lg border-[1.5px] text-left cursor-pointer disabled:cursor-not-allowed transition-[background-color,border-color,scale] duration-200 active:scale-99',
                    active ? 'border-accent bg-accent-soft' : 'border-line bg-surface hover:bg-surface-hover',
                  )}
                >
                  <div className={cn(
                    'w-4 h-4 rounded-full border-[1.5px] flex items-center justify-center shrink-0 mt-0.5 transition-colors duration-200',
                    active ? 'border-accent bg-accent' : 'border-line bg-transparent',
                  )}>
                    {active && <CheckIcon size={9} className="text-accent-fg" weight="bold" />}
                  </div>

                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-1.5">
                      <span className={cn('text-[12.5px] font-semibold leading-tight truncate transition-colors duration-200', active ? 'text-accent' : 'text-fg')}>
                        {displayName}
                      </span>
                      {isRecommended && (
                        <span className="shrink-0 text-[8.5px] font-bold text-accent bg-accent-soft border border-[color-mix(in_srgb,var(--accent)_30%,transparent)] rounded-xs px-1 py-px uppercase tracking-wider">
                          Best
                        </span>
                      )}
                    </div>
                    <p className="text-[10.5px] text-muted leading-snug truncate">
                      {description}
                    </p>
                    <div className="flex items-center gap-2 mt-1 text-[10px] text-muted">
                      <span className="flex items-center gap-1">
                        <HardDrivesIcon size={10} />
                        <span className="tabular-nums">{formatModelSize(sizeBytes)}</span>
                      </span>
                      <span className="flex items-center gap-1">
                        <GlobeIcon size={10} />
                        {multilingual ? 'Multilingual' : 'English'}
                      </span>
                      {streaming && (
                        <span className="flex items-center gap-1 text-accent font-semibold">
                          <BroadcastIcon size={10} weight="bold" />
                          Live
                        </span>
                      )}
                    </div>
                  </div>
                </button>
              )
            })}
          </div>
        </div>

        <div className="shrink-0 px-6 py-3 border-t border-line-soft">
          {confirmed ? (
            <div className="flex flex-col gap-2 transition-opacity starting:opacity-0">
              <div className="flex items-center justify-between text-[11px] mb-1">
                <span className="text-fg-2">
                  {download ? 'Downloading model…' : 'Download complete — loading…'}
                </span>
                <span className="text-accent font-semibold tabular-nums">
                  {download?.progress ?? 100}%
                </span>
              </div>
              <div className="h-0.75 rounded-full bg-line overflow-hidden">
                <div
                  className="h-full rounded-full bg-accent transition-[width] duration-300 ease-linear starting:w-0"
                  style={{ width: `${download?.progress ?? 100}%` }}
                />
              </div>
            </div>
          ) : (
            <div className="flex items-center justify-between gap-4 transition-opacity starting:opacity-0">
              <p className={`text-[11px] min-w-0 truncate ${loadError ? 'text-danger' : 'text-muted'}`}>
                {loadError ??
                  (selectedModel
                    ? `${selectedModel.displayName} · ${formatModelSize(selectedModel.sizeBytes)} download`
                    : 'Select a model to continue')}
              </p>
              {loadError && catalog.length === 0 ? (
                <Button
                  variant="primary"
                  className="shrink-0 min-w-44"
                  onClick={() => setReloadKey(k => k + 1)}
                >
                  <ArrowsClockwiseIcon size={12} weight="bold" />
                  Try again
                </Button>
              ) : (
              <Button
                variant="tinted"
                className="shrink-0 min-w-44"
                onClick={handleConfirm}
                disabled={confirming || !selected}
              >
                {confirming ? (
                  <>
                    <CircleNotchIcon size={12} className="animate-spin" weight="bold" />
                    Starting download…
                  </>
                ) : (
                  <>
                    <DownloadSimpleIcon size={12} weight="bold" />
                    Download &amp; continue
                  </>
                )}
              </Button>
              )}
            </div>
          )}
        </div>
      </div>
    </div>,
    document.body
  )
}
