import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { AnimatePresence, motion } from 'framer-motion'
import { Box, Cpu, HardDrive, Zap } from 'lucide-react'
import { COMMANDS } from '../lib/commands'
import { MODEL_OPTIONS, recommendedToVariant, type ModelVariant } from '../lib/models'
import { useAppStore } from '../store/useAppStore'
import { Button } from '@/components/ui/button'
import type { HardwareProfile, ModelInfo } from '../types'

export function ModelPickerModal() {
  const { setModelChosen, modelDownloading, downloadProgress } = useAppStore()
  const [profile, setProfile] = useState<HardwareProfile | null>(null)
  const [selected, setSelected] = useState<ModelVariant>('parakeet-tdt-0.6b-v3')
  const [confirming, setConfirming] = useState(false)
  const [confirmed, setConfirmed] = useState(false)

  useEffect(() => {
    invoke<ModelInfo>(COMMANDS.GET_MODEL_INFO).then(info => { if (info.downloaded) setModelChosen(true) }).catch(() => {})
    invoke<HardwareProfile>(COMMANDS.GET_HARDWARE_PROFILE).then(p => {
      setProfile(p)
      setSelected(recommendedToVariant(p.recommendedModel))
    }).catch(() => {})
  }, [setModelChosen])

  const handleConfirm = async () => {
    setConfirming(true)
    try {
      await invoke(COMMANDS.SET_MODEL_OVERRIDE, { variant: selected })
      const info = await invoke<ModelInfo>(COMMANDS.GET_MODEL_INFO)
      if (info.downloaded) return setModelChosen(true)
      await invoke(COMMANDS.RETRY_MODEL_DOWNLOAD)
      setConfirmed(true)
    } catch { setConfirming(false) }
  }

  useEffect(() => {
    if (confirmed && !modelDownloading && downloadProgress === 100) setModelChosen(true)
  }, [confirmed, modelDownloading, downloadProgress, setModelChosen])

  const recommended = profile ? recommendedToVariant(profile.recommendedModel) : null
  return (
    <div className="fixed top-8 inset-x-0 bottom-0 z-50 flex items-center justify-center bg-black/15">
      <motion.div initial={{ opacity: 0, scale: 0.96, y: 8 }} animate={{ opacity: 1, scale: 1, y: 0 }} className="w-130 max-h-[90vh] flex flex-col bg-(--panel) border border-(--border) rounded-(--r-xl) shadow-(--shadow-lg) overflow-hidden">
        <div className="shrink-0 px-6 pt-5 pb-4 border-b border-(--border-soft)">
          <div className="flex items-center gap-3 mb-3"><div className="w-8 h-8 rounded-(--r-md) bg-(--accent) flex items-center justify-center text-primary-foreground"><Zap size={14} /></div><div><h2 className="text-[15px] font-bold text-(--fg) m-0">Choose your NVIDIA model</h2><p className="text-[11px] text-muted-foreground mt-px">You can change this later in Settings.</p></div></div>
          {profile && <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-(--r-md) bg-(--surface) border border-(--border-soft) w-fit"><Cpu size={10} /><span className="text-[10px] text-(--fg-2)">{profile.gpuName} · {profile.executionProvider.toUpperCase()}{profile.vramGb > 0 ? ` · ${profile.vramGb} GB VRAM` : ''}</span></div>}
        </div>
        <div className="flex-1 min-h-0 overflow-y-auto px-6 py-4 grid grid-cols-2 gap-2">
          {MODEL_OPTIONS.map(model => {
            const active = selected === model.value
            return <motion.button key={model.value} type="button" disabled={confirmed} onClick={() => setSelected(model.value)} className="relative w-full flex items-start gap-2.5 px-3 py-3 rounded-(--r-md) border-[1.5px] text-left cursor-pointer disabled:cursor-not-allowed" animate={{ backgroundColor: active ? 'var(--accent-soft)' : 'var(--surface)', borderColor: active ? 'var(--accent)' : 'var(--border)' }}>
              {recommended === model.value && <div className="absolute top-0 right-0 text-(--accent) text-[8px] font-extrabold uppercase px-2 py-1">Recommended</div>}
              <Box size={16} className="shrink-0 mt-0.5" style={{ color: active ? 'var(--accent)' : 'var(--fg-2)' }} />
              <div className="min-w-0"><span className="text-[10px] font-bold uppercase text-muted-foreground">{model.tier}</span><div className="text-[12.5px] font-semibold text-(--fg)">{model.label}</div><p className="text-[10px] text-(--fg-2) mt-0.5">{model.description}</p><span className="flex items-center gap-1 text-[10px] text-muted-foreground mt-1"><HardDrive size={11} />{model.sizeLabel}</span></div>
            </motion.button>
          })}
        </div>
        <div className="shrink-0 px-6 pb-5 pt-3 border-t border-(--border-soft)">
          <AnimatePresence mode="wait">{confirmed ? <motion.div key="downloading" initial={{ opacity: 0 }} animate={{ opacity: 1 }}><div className="flex justify-between text-[11px] mb-2"><span>Downloading model…</span><span className="text-(--accent)">{downloadProgress}%</span></div><div className="h-0.75 rounded-full bg-(--border) overflow-hidden"><div className="h-full bg-(--accent)" style={{ width: `${downloadProgress}%` }} /></div></motion.div> : <Button className="w-full" onClick={handleConfirm} disabled={confirming}>{confirming ? 'Starting download…' : `Download ${MODEL_OPTIONS.find(m => m.value === selected)?.tier}`}</Button>}</AnimatePresence>
        </div>
      </motion.div>
    </div>
  )
}
