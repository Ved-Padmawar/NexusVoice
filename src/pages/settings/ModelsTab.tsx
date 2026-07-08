import { useCallback, useEffect, useState } from 'react'
import { motion } from 'framer-motion'
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'sonner'
import { Box, Check, Cpu, Database, Download, HardDrive, Trash2 } from 'lucide-react'
import { COMMANDS } from '../../lib/commands'
import { MODEL_OPTIONS, recommendedToVariant, type ModelOption } from '../../lib/models'
import type { HardwareProfile } from '../../types'

type DownloadedModel = { variant: string; sizeBytes: number; isActive: boolean }

function formatBytes(bytes: number): string {
  if (bytes <= 0) return '0 MB'
  return bytes >= 1e9 ? `${(bytes / 1e9).toFixed(2)} GB` : `${Math.round(bytes / 1e6)} MB`
}

export function ModelsTab() {
  const [profile, setProfile] = useState<HardwareProfile | null>(null)
  const [activeVariant, setActiveVariant] = useState<string>('parakeet-tdt-0.6b-v3')
  const [downloaded, setDownloaded] = useState<Record<string, number>>({})
  const [busy, setBusy] = useState(false)

  const refresh = useCallback(async () => {
    const models = await invoke<DownloadedModel[]>(COMMANDS.GET_DOWNLOADED_MODELS).catch(() => [])
    const map: Record<string, number> = {}
    for (const model of models) {
      map[model.variant] = model.sizeBytes
      if (model.isActive) setActiveVariant(model.variant)
    }
    setDownloaded(map)
  }, [])

  useEffect(() => {
    let cancelled = false
    const load = async () => {
      const profile = await invoke<HardwareProfile>(COMMANDS.GET_HARDWARE_PROFILE).catch(() => null)
      if (!cancelled && profile) setProfile(profile)
      if (!cancelled) await refresh()
    }
    void load()
    return () => {
      cancelled = true
    }
  }, [refresh])

  const selectModel = async (model: ModelOption) => {
    if (busy || activeVariant === model.value) return
    setBusy(true)
    try {
      await invoke(COMMANDS.SET_MODEL_OVERRIDE, { variant: model.value })
      setActiveVariant(model.value)
      if (!(model.value in downloaded)) await invoke(COMMANDS.RETRY_MODEL_DOWNLOAD)
      await refresh()
      toast.success(`Selected ${model.tier}`)
    } catch { toast.error(`Could not select ${model.tier}`) }
    finally { setBusy(false) }
  }

  const downloadModel = async (model: ModelOption) => {
    if (busy) return
    setBusy(true)
    try {
      await invoke(COMMANDS.SET_MODEL_OVERRIDE, { variant: model.value })
      setActiveVariant(model.value)
      await invoke(COMMANDS.RETRY_MODEL_DOWNLOAD)
      await refresh()
    } catch { toast.error('Could not start model download') }
    finally { setBusy(false) }
  }

  const deleteModel = async (model: ModelOption) => {
    if (busy) return
    setBusy(true)
    try {
      await invoke(COMMANDS.DELETE_MODEL, { variant: model.value })
      await refresh()
      toast.success(`Deleted ${model.tier}`)
    } catch { toast.error('Could not delete model') }
    finally { setBusy(false) }
  }

  const recommended = profile ? recommendedToVariant(profile.recommendedModel) : null
  const activeModel = MODEL_OPTIONS.find(model => model.value === activeVariant)
  const totalBytes = Object.values(downloaded).reduce((sum, bytes) => sum + bytes, 0)

  return <div className="flex flex-col gap-4">
    <div className="flex items-start justify-between gap-4">
      <div><p className="text-[12px] font-semibold text-[var(--fg-2)]">NVIDIA speech-to-text model</p><p className="text-[11px] text-[var(--muted)] mt-[3px] flex items-center gap-1"><Cpu size={10} />{profile ? `${profile.gpuName} · ${profile.executionProvider.toUpperCase()}${profile.vramGb > 0 ? ` · ${profile.vramGb}GB VRAM` : ''}` : 'Detecting hardware…'}</p></div>
      <div className="flex items-center gap-2"><span className="text-[11px] font-semibold text-[var(--accent)] bg-[var(--accent-soft)] border border-[var(--accent)] px-2.5 py-[5px] rounded-[var(--r-md)]">{activeModel?.label ?? activeVariant}</span><span className="flex items-center gap-1.5 text-[11px] text-[var(--fg-2)] bg-[var(--surface)] border border-[var(--border-soft)] px-2.5 py-[5px] rounded-[var(--r-md)]"><Database size={12} /><b>{Object.keys(downloaded).length}</b> of {MODEL_OPTIONS.length} · {formatBytes(totalBytes)}</span></div>
    </div>
    <div className="grid gap-2.5" style={{ gridTemplateColumns: 'repeat(3, minmax(0, 1fr))' }}>
      {MODEL_OPTIONS.map(model => {
        const active = activeVariant === model.value
        const isDownloaded = model.value in downloaded
        return <motion.div key={model.value} role="button" tabIndex={0} className="relative flex flex-col gap-2 min-w-0 p-3.5 rounded-[var(--r-lg)] border-[1.5px] cursor-pointer" animate={{ backgroundColor: active ? 'var(--accent-soft)' : 'var(--surface)', borderColor: active ? 'var(--accent)' : 'var(--border)' }} onClick={() => selectModel(model)} onKeyDown={e => { if (e.key === 'Enter' || e.key === ' ') void selectModel(model) }}>
          {recommended === model.value && <div className="absolute top-0 right-0 text-[8px] font-extrabold uppercase text-[var(--accent)] px-2 py-1">Recommended</div>}
          <div className="flex items-center gap-2.5"><Box size={18} style={{ color: active ? 'var(--accent)' : 'var(--fg-2)' }} /><div><span className="text-[10px] uppercase font-bold text-[var(--muted)]">{model.tier}</span><div className="text-[13px] font-bold text-[var(--fg)]">{model.label}</div></div></div>
          <div className="text-[11px] text-[var(--fg-2)] min-h-[34px]">{model.description}</div>
          <div className="flex items-center justify-between gap-2 mt-auto pt-2.5 border-t border-[var(--border-soft)]"><span className="inline-flex items-center gap-1.5 text-[10.5px] text-[var(--muted)]"><HardDrive size={12} />{isDownloaded ? formatBytes(downloaded[model.value]) : model.sizeLabel}</span>{isDownloaded ? <div className="flex items-center gap-1.5"><span className="inline-flex items-center gap-1 text-[10.5px] text-[var(--success)]"><Check size={13} />Downloaded</span><button type="button" disabled={busy || active} title={active ? 'Active model cannot be deleted' : 'Delete model'} className="w-[26px] h-[26px] rounded-[var(--r-md)] flex items-center justify-center bg-transparent border border-[var(--border)] disabled:opacity-30" onClick={e => { e.stopPropagation(); void deleteModel(model) }}><Trash2 size={13} /></button></div> : <button type="button" disabled={busy} className="inline-flex items-center gap-1.5 px-2.5 h-[26px] rounded-[var(--r-md)] bg-[var(--accent)] text-[var(--accent-fg)] border-none text-[10.5px] font-semibold" onClick={e => { e.stopPropagation(); void downloadModel(model) }}><Download size={12} />Download</button>}</div>
        </motion.div>
      })}
    </div>
  </div>
}
