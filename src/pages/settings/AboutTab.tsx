import { useState, useCallback, useRef, useEffect } from 'react'
import { motion } from 'framer-motion'
import { invoke } from '@tauri-apps/api/core'
import {
  AlertCircle, CheckCircle2, HardDrive, MemoryStick, Monitor,
  RefreshCw, Download, ArrowUpCircle, Cpu, Shield, Globe,
} from 'lucide-react'
import { check } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'
import { Button } from '@/components/ui/button'
import { COMMANDS } from '../../lib/commands'
import type { HardwareProfile } from '../../types'
import { extractErrorMessage } from '../../lib/errors'

type UpdateStatus = 'idle' | 'checking' | 'available' | 'downloading' | 'ready' | 'error' | 'up-to-date'

type DownloadedModel = {
  variant: string
  displayName: string
  sizeBytes: number
  isActive: boolean
}

function formatBytes(bytes: number): string {
  if (bytes >= 1_073_741_824) return `${(bytes / 1_073_741_824).toFixed(1)} GB`
  if (bytes >= 1_048_576) return `${Math.round(bytes / 1_048_576)} MB`
  return `${Math.max(1, Math.round(bytes / 1024))} KB`
}

/** One label/value row in the system-info card. */
function InfoRow({ Icon, label, value }: { Icon: typeof Cpu; label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-4 px-4 py-2.5">
      <span className="flex items-center gap-2 text-[12px] text-(--fg-2)">
        <Icon size={13} strokeWidth={1.75} className="text-(--muted) shrink-0" />
        {label}
      </span>
      <span className="text-[12px] text-(--muted) truncate text-right">{value}</span>
    </div>
  )
}

export function AboutTab() {
  const [updateStatus, setUpdateStatus] = useState<UpdateStatus>('idle')
  const [updateVersion, setUpdateVersion] = useState<string | null>(null)
  const [downloadProgress, setDownloadProgress] = useState(0)
  const [updateError, setUpdateError] = useState<string | null>(null)
  const [profile, setProfile] = useState<HardwareProfile | null>(null)
  const [onDisk, setOnDisk] = useState<DownloadedModel[]>([])
  const updaterRef = useRef<Awaited<ReturnType<typeof check>> | null>(null)

  useEffect(() => {
    invoke<HardwareProfile>(COMMANDS.GET_HARDWARE_PROFILE).then(setProfile).catch(() => {})
    invoke<DownloadedModel[]>(COMMANDS.GET_DOWNLOADED_MODELS).then(setOnDisk).catch(() => {})
  }, [])

  const modelBytes = onDisk.reduce((acc, m) => acc + m.sizeBytes, 0)

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
      setUpdateError(extractErrorMessage(e, 'Update check failed'))
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
      setUpdateError(extractErrorMessage(e, 'Download failed'))
      setUpdateStatus('error')
    }
  }, [])

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

      {/* System */}
      <div className="overflow-hidden rounded-(--r-lg) border border-(--border-soft) bg-(--panel)">
        <div className="px-4 py-2.5 border-b border-(--border-soft) text-[10px] font-semibold uppercase tracking-[0.08em] text-(--muted)">
          System
        </div>
        <div className="divide-y divide-(--border-soft)">
          <InfoRow
            Icon={Monitor}
            label="Compute"
            value={profile ? `${profile.gpuName} · ${profile.executionProvider.toUpperCase()}` : 'Detecting…'}
          />
          <InfoRow
            Icon={MemoryStick}
            label="Memory"
            value={
              profile
                ? `${profile.ramGb} GB RAM${profile.vramGb > 0 ? ` · ${profile.vramGb} GB VRAM` : ''}`
                : 'Detecting…'
            }
          />
          <InfoRow
            Icon={HardDrive}
            label="Models on disk"
            value={
              onDisk.length > 0
                ? `${onDisk.length} model${onDisk.length > 1 ? 's' : ''} · ${formatBytes(modelBytes)}`
                : 'None downloaded'
            }
          />
        </div>
      </div>

      {/* Updates */}
      <div className="flex flex-col gap-3 pt-2 border-t border-[var(--border-soft)]">
        <p className="text-[12px] font-semibold text-[var(--fg-2)] tracking-[-0.01em]">Updates</p>

        <div className={`flex items-center justify-between gap-4 px-4 py-3 rounded-[var(--r-lg)] bg-[var(--surface)] border ${
          updateStatus === 'available' || updateStatus === 'downloading' || updateStatus === 'checking'
            ? 'border-[var(--accent)]'
            : updateStatus === 'ready'
              ? 'border-[var(--success)]'
              : updateStatus === 'error'
                ? 'border-[var(--danger)]'
                : 'border-[var(--border-soft)]'
        }`}>
          {/* Icon badge */}
          <div className="flex items-center gap-3 flex-1 min-w-0">
            <div className={`w-8 h-8 rounded-[var(--r-md)] flex items-center justify-center flex-shrink-0 ${
              updateStatus === 'up-to-date' || updateStatus === 'ready'
                ? 'bg-[var(--success-soft)] text-[var(--success)]'
                : updateStatus === 'error'
                  ? 'bg-[var(--danger-soft)] text-[var(--danger)]'
                  : 'bg-[var(--accent-soft)] text-[var(--accent)]'
            }`}>
              {(updateStatus === 'up-to-date') && <CheckCircle2 size={14} strokeWidth={2} />}
              {(updateStatus === 'ready') && <CheckCircle2 size={14} strokeWidth={2} />}
              {(updateStatus === 'error') && <AlertCircle size={14} strokeWidth={2} />}
              {(updateStatus === 'idle' || updateStatus === 'checking') && <motion.span animate={updateStatus === 'checking' ? { rotate: 360 } : {}} transition={{ duration: 1, ease: 'linear', repeat: Infinity }}><RefreshCw size={14} strokeWidth={2} /></motion.span>}
              {(updateStatus === 'available' || updateStatus === 'downloading') && <Download size={14} strokeWidth={2} />}
            </div>

            {/* Text + progress */}
            <div className="flex-1 min-w-0">
              {updateStatus === 'downloading' ? (
                <>
                  <div className="flex items-center gap-3">
                    <span className="text-[12px] font-medium text-[var(--fg-2)]">Downloading…</span>
                    <span className="text-[11px] font-semibold text-[var(--accent)] tabular-nums">{downloadProgress}%</span>
                  </div>
                  <div className="h-[3px] rounded-full bg-[var(--border-soft)] overflow-hidden mt-[6px]">
                    <div
                      className="h-full rounded-full bg-[var(--accent)] transition-[width] duration-300 ease-linear"
                      style={{ width: `${downloadProgress}%` }}
                    />
                  </div>
                </>
              ) : (
                <>
                  <p className={`text-[12px] font-medium ${
                    updateStatus === 'error' ? 'text-[var(--danger)]'
                      : updateStatus === 'up-to-date' || updateStatus === 'ready' ? 'text-[var(--success)]'
                        : updateStatus === 'available' ? 'text-[var(--fg)]'
                          : 'text-[var(--fg)]'
                  }`}>
                    {updateStatus === 'idle' ? 'Check for updates' : updateStatus === 'checking' ? 'Looking for updates…' : updateStatus === 'up-to-date' ? "You're up to date" : updateStatus === 'available' ? `v${updateVersion} available` : updateStatus === 'ready' ? 'Ready to install' : updateError ?? 'Update failed'}
                  </p>
                  <p className="text-[10px] text-[var(--muted)] mt-[2px]">
                    {updateStatus === 'idle' ? `Currently on v${__APP_VERSION__}` : updateStatus === 'checking' ? 'Please wait…' : updateStatus === 'up-to-date' ? `v${__APP_VERSION__} is the latest` : updateStatus === 'available' ? 'Ready to download' : updateStatus === 'ready' ? `Restart to apply v${updateVersion}` : 'Check your network connection'}
                  </p>
                </>
              )}
            </div>
          </div>

          {/* Action button */}
          <div className="flex-shrink-0">
            {(updateStatus === 'idle' || updateStatus === 'up-to-date' || updateStatus === 'error') && (
              <Button size="sm" onClick={checkForUpdate}>
                <RefreshCw size={11} strokeWidth={2} />
                {updateStatus === 'up-to-date' ? 'Check again' : updateStatus === 'error' ? 'Retry' : 'Check'}
              </Button>
            )}
            {updateStatus === 'checking' && (
              <Button size="sm" disabled>
                <motion.span animate={{ rotate: 360 }} transition={{ duration: 1, ease: 'linear', repeat: Infinity }}><RefreshCw size={11} strokeWidth={2} /></motion.span>
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
                Downloading…
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
