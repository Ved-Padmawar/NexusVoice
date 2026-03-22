import { useState, useCallback, useRef, useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { COMMANDS } from '../../lib/commands'
import { MODEL_OPTIONS, modelNameToOverride, recommendedToOverride, type ModelOverride } from '../../lib/models'
import { toast } from 'sonner'
import {
  AlertCircle, CheckCircle2,
  RefreshCw, Download, ArrowUpCircle, Cpu, Shield, Globe,
} from 'lucide-react'
import { check } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'
import { Button } from '@/components/ui/button'
import type { HardwareProfile } from '../../types'

type UpdateStatus = 'idle' | 'checking' | 'available' | 'downloading' | 'ready' | 'error' | 'up-to-date'

export function AboutTab() {
  const [profile, setProfile] = useState<HardwareProfile | null>(null)
  const [selected, setSelected] = useState<ModelOverride>('large')
  const [activeModelName, setActiveModelName] = useState<string | null>(null)
  const [modelSaving, setModelSaving] = useState(false)

  const [updateStatus, setUpdateStatus] = useState<UpdateStatus>('idle')
  const [updateVersion, setUpdateVersion] = useState<string | null>(null)
  const [downloadProgress, setDownloadProgress] = useState(0)
  const [updateError, setUpdateError] = useState<string | null>(null)
  const updaterRef = useRef<Awaited<ReturnType<typeof check>> | null>(null)

  useEffect(() => {
    invoke<HardwareProfile>(COMMANDS.GET_HARDWARE_PROFILE).then(setProfile).catch(() => {})
    invoke<{ modelName: string }>(COMMANDS.GET_MODEL_INFO).then(info => {
      setActiveModelName(info.modelName)
      setSelected(modelNameToOverride(info.modelName))
    }).catch(() => {})
  }, [])

  const handleModelChange = async (v: ModelOverride) => {
    setSelected(v)
    setModelSaving(true)
    try {
      await invoke(COMMANDS.SET_MODEL_OVERRIDE, { variant: v })
      invoke(COMMANDS.RETRY_MODEL_DOWNLOAD).catch(() => {})
      const info = await invoke<{ modelName: string }>(COMMANDS.GET_MODEL_INFO)
      setActiveModelName(info.modelName)
      toast.success('Model updated')
    } catch { /* ignore */ }
    finally { setModelSaving(false) }
  }

  const checkForUpdate = useCallback(async () => {
    setUpdateStatus('checking')
    setUpdateError(null)
    try {
      const update = await check()
      if (update?.available) {
        updaterRef.current = update
        setUpdateVersion(update.version)
        setUpdateStatus('available')
      } else {
        setUpdateStatus('up-to-date')
      }
    } catch (e) {
      setUpdateError(e instanceof Error ? e.message : 'Update check failed')
      setUpdateStatus('error')
    }
  }, [])

  const downloadAndInstall = useCallback(async () => {
    const update = updaterRef.current
    if (!update) return
    setUpdateStatus('downloading')
    setDownloadProgress(0)
    try {
      let downloaded = 0
      let total = 0
      await update.downloadAndInstall((progress) => {
        if (progress.event === 'Started') {
          total = progress.data.contentLength ?? 0
        } else if (progress.event === 'Progress') {
          downloaded += progress.data.chunkLength
          if (total > 0) setDownloadProgress(Math.round((downloaded / total) * 100))
        } else if (progress.event === 'Finished') {
          setDownloadProgress(100)
          setUpdateStatus('ready')
        }
      })
    } catch (e) {
      setUpdateError(e instanceof Error ? e.message : 'Download failed')
      setUpdateStatus('error')
    }
  }, [])

  const updateDesc = {
    idle: 'Check for the latest release.',
    checking: 'Checking for updates…',
    'up-to-date': 'You\'re on the latest version.',
    available: `v${updateVersion} is ready to download.`,
    downloading: `Downloading update… ${downloadProgress}%`,
    ready: 'Restart to apply the update.',
    error: updateError ?? 'Something went wrong.',
  }[updateStatus]

  return (
    <div className="flex flex-col gap-4">

      {/* Info pills */}
      <div className="flex gap-2">
        {[
          { Icon: Cpu,    label: 'whisper-rs (ggml)' },
          { Icon: Globe,  label: 'English' },
          { Icon: Shield, label: '100% on-device' },
        ].map(({ Icon, label }) => (
          <div key={label} className="flex items-center gap-[6px] px-3 py-[6px] rounded-[var(--r-md)] bg-[var(--surface)] border border-[var(--border-soft)] text-[11px] text-[var(--fg-2)]">
            <Icon size={11} strokeWidth={1.75} className="text-[var(--muted)] flex-shrink-0" />
            {label}
          </div>
        ))}
      </div>

      {/* Model selector */}
      <div className="flex flex-col gap-3 pt-2 border-t border-[var(--border-soft)]">
        <div className="flex items-center justify-between">
          <div>
            <p className="text-[11px] font-semibold text-[var(--fg-2)] uppercase tracking-[0.03em]">Whisper Model</p>
            <p className="text-[11px] text-[var(--muted)] mt-[3px]">
              {profile
                ? <span className="flex items-center gap-1"><Cpu size={10} strokeWidth={1.75} />{profile.gpuName} · {profile.executionProvider.toUpperCase()}{profile.vramGb > 0 ? ` · ${profile.vramGb}GB VRAM` : ''}</span>
                : 'Detecting hardware…'}
            </p>
          </div>
          <div className="flex items-center gap-2">
            {activeModelName && (
              <span className="text-[10px] font-semibold text-[var(--accent)] bg-[var(--accent-soft)] border border-[var(--accent-soft)] px-[6px] py-px rounded-[var(--r-sm)]">
                {MODEL_OPTIONS.find(m => m.value === modelNameToOverride(activeModelName))?.label ?? activeModelName}
              </span>
            )}
          </div>
        </div>

        <div className="flex gap-2">
          {MODEL_OPTIONS.map(({ value, label, description }) => {
            const isRecommended = profile && recommendedToOverride(profile.recommendedModel) === value
            const active = selected === value
            return (
              <button
                key={value}
                type="button"
                className={`flex-1 flex flex-col items-start gap-[3px] px-3 py-[10px] rounded-[var(--r-md)] border-[1.5px] cursor-pointer transition-all duration-[var(--t-fast)] disabled:opacity-50 disabled:cursor-not-allowed ${active ? 'border-[var(--accent)] bg-[var(--accent-soft)]' : 'border-[var(--border)] bg-[var(--surface)] hover:border-[var(--accent)] hover:bg-[var(--surface-hover)]'}`}
                onClick={() => handleModelChange(value)}
                disabled={modelSaving}
              >
                <div className="flex items-center gap-[6px]">
                  <span className={`text-[12px] font-semibold ${active ? 'text-[var(--accent)]' : 'text-[var(--fg)]'}`}>{label}</span>
                  {isRecommended && (
                    <span className="text-[9px] font-bold text-[var(--accent)] bg-[var(--accent-soft)] rounded-[var(--r-xs)] px-[5px] py-px uppercase tracking-[0.04em]">
                      Recommended
                    </span>
                  )}
                </div>
                <span className="text-[10px] text-[var(--muted)]">{description}</span>
              </button>
            )
          })}
        </div>
      </div>

      {/* Updates */}
      <div className="flex flex-col gap-3 pt-2 border-t border-[var(--border-soft)]">
        <p className="text-[11px] font-semibold text-[var(--fg-2)] uppercase tracking-[0.03em]">Updates</p>

        <div className="flex items-center justify-between gap-4 px-4 py-3 rounded-[var(--r-lg)] bg-[var(--surface)] border border-[var(--border-soft)]">
          <div className="flex-1 min-w-0">
            <p className={`text-[12px] font-medium ${updateStatus === 'error' ? 'text-[var(--danger)]' : updateStatus === 'up-to-date' || updateStatus === 'ready' ? 'text-[var(--success)]' : 'text-[var(--fg)]'}`}>
              {updateDesc}
            </p>
            {updateStatus === 'downloading' && (
              <div className="h-[2px] rounded-full bg-[var(--border)] overflow-hidden mt-2">
                <div
                  className="h-full rounded-full bg-[var(--accent)] transition-[width] duration-300 ease-linear"
                  style={{ width: `${downloadProgress}%` }}
                />
              </div>
            )}
          </div>

          <div className="flex items-center gap-2 flex-shrink-0">
            {updateStatus === 'up-to-date' && <CheckCircle2 size={14} strokeWidth={2} className="text-[var(--success)]" />}
            {updateStatus === 'error' && <AlertCircle size={14} strokeWidth={2} className="text-[var(--danger)]" />}
            {(updateStatus === 'idle' || updateStatus === 'up-to-date' || updateStatus === 'error') && (
              <Button size="sm" variant="outline" onClick={checkForUpdate}>
                <RefreshCw size={11} strokeWidth={2} />
                Check
              </Button>
            )}
            {updateStatus === 'checking' && (
              <Button size="sm" variant="outline" disabled>
                <RefreshCw size={11} strokeWidth={2} className="animate-[spin_1s_linear_infinite]" />
                Checking…
              </Button>
            )}
            {updateStatus === 'available' && (
              <Button size="sm" onClick={downloadAndInstall}>
                <Download size={11} strokeWidth={2} />
                Download
              </Button>
            )}
            {updateStatus === 'downloading' && (
              <Button size="sm" disabled>
                <Download size={11} strokeWidth={2} />
                {downloadProgress}%
              </Button>
            )}
            {updateStatus === 'ready' && (
              <Button size="sm" onClick={() => relaunch()}>
                <ArrowUpCircle size={11} strokeWidth={2} />
                Restart
              </Button>
            )}
          </div>
        </div>
      </div>

    </div>
  )
}
