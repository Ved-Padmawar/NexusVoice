import { useState, useEffect } from 'react'
import { motion } from 'framer-motion'
import { invoke } from '@tauri-apps/api/core'
import {
  AlertCircle, CheckCircle2, HardDrive, MemoryStick, Mic, Monitor,
  RefreshCw, Download, ArrowUpCircle, Cpu, Shield,
} from 'lucide-react'
import { relaunch } from '@tauri-apps/plugin-process'
import { Button } from '@/components/ui/button'
import { COMMANDS } from '../../lib/commands'
import type { HardwareProfile } from '../../types'
import { useAppStore } from '../../store/useAppStore'

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
        <Icon size={13} strokeWidth={1.75} className="text-muted-foreground shrink-0" />
        {label}
      </span>
      <span className="text-[12px] text-muted-foreground truncate text-right">{value}</span>
    </div>
  )
}

export function AboutTab() {
  // Update state lives in the store so this tab and the sidebar prompt drive
  // one install rather than each holding a private updater handle.
  const updateStatus = useAppStore(s => s.updateStatus)
  const updateVersion = useAppStore(s => s.updateVersion)
  const downloadProgress = useAppStore(s => s.updateProgress)
  const updateError = useAppStore(s => s.updateError)
  const checkForUpdate = useAppStore(s => s.checkForUpdate)
  const downloadAndInstall = useAppStore(s => s.installUpdate)
  const [profile, setProfile] = useState<HardwareProfile | null>(null)
  const [onDisk, setOnDisk] = useState<DownloadedModel[]>([])

  useEffect(() => {
    invoke<HardwareProfile>(COMMANDS.GET_HARDWARE_PROFILE).then(setProfile).catch(() => {})
    invoke<DownloadedModel[]>(COMMANDS.GET_DOWNLOADED_MODELS).then(setOnDisk).catch(() => {})
  }, [])

  const modelBytes = onDisk.reduce((acc, m) => acc + m.sizeBytes, 0)

  return (
    <div className="flex flex-col gap-4">

      {/* Version hero, then what the app is. */}
      <div className="flex items-center gap-3.5 rounded-(--r-lg) border border-(--border-soft) bg-(--panel) p-4">
        <span className="grid size-11 shrink-0 place-items-center rounded-(--r-lg) bg-(--accent-soft) text-(--accent)">
          <Mic size={20} strokeWidth={1.9} />
        </span>
        <div className="min-w-0 flex-1">
          <div className="flex items-baseline gap-2">
            <span className="text-[16px] font-bold tracking-[-0.025em] text-(--fg)">NexusVoice</span>
            <span className="text-[12px] font-medium tabular-nums text-muted-foreground">
              v{__APP_VERSION__}
            </span>
          </div>
          <p className="mt-0.5 text-[11px] text-muted-foreground">
            Local speech to text. Audio never leaves this machine.
          </p>
        </div>
        <div className="flex shrink-0 gap-2">
          {[
            { Icon: Cpu, label: 'transcribe-cpp' },
            { Icon: Shield, label: '100% on-device' },
          ].map(({ Icon, label }) => (
            <span key={label} className="flex items-center gap-1.5 rounded-(--r-md) border border-(--border-soft) bg-(--surface) px-2.5 py-1.5 text-[11px] text-(--fg-2)">
              <Icon size={11} strokeWidth={1.75} className="shrink-0 text-muted-foreground" />
              {label}
            </span>
          ))}
        </div>
      </div>

      {/* System */}
      <div className="overflow-hidden rounded-(--r-lg) border border-(--border-soft) bg-(--panel)">
        <div className="px-4 py-2.5 border-b border-(--border-soft) text-[10px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">
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
      <div className="overflow-hidden rounded-(--r-lg) border border-(--border-soft) bg-(--panel)">
        <div className="px-4 py-2.5 border-b border-(--border-soft) text-[10px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">
          Updates
        </div>

        <div className={`m-3 flex items-center justify-between gap-4 px-3 py-2.5 rounded-(--r-md) bg-(--surface) border ${
          updateStatus === 'available' || updateStatus === 'downloading' || updateStatus === 'checking'
            ? 'border-(--accent)'
            : updateStatus === 'ready'
              ? 'border-(--success)'
              : updateStatus === 'error'
                ? 'border-destructive'
                : 'border-(--border-soft)'
        }`}>
          {/* Icon badge */}
          <div className="flex items-center gap-3 flex-1 min-w-0">
            <div className={`w-8 h-8 rounded-(--r-md) flex items-center justify-center shrink-0 ${
              updateStatus === 'up-to-date' || updateStatus === 'ready'
                ? 'bg-(--success-soft) text-(--success)'
                : updateStatus === 'error'
                  ? 'bg-(--danger-soft) text-destructive'
                  : 'bg-(--accent-soft) text-(--accent)'
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
                    <span className="text-[12px] font-medium text-(--fg-2)">Downloading…</span>
                    <span className="text-[11px] font-semibold text-(--accent) tabular-nums">{downloadProgress}%</span>
                  </div>
                  <div className="h-0.75 rounded-full bg-(--border-soft) overflow-hidden mt-1.5">
                    <div
                      className="h-full rounded-full bg-(--accent) transition-[width] duration-300 ease-linear"
                      style={{ width: `${downloadProgress}%` }}
                    />
                  </div>
                </>
              ) : (
                <>
                  <p className={`text-[12px] font-medium ${
                    updateStatus === 'error' ? 'text-destructive'
                      : updateStatus === 'up-to-date' || updateStatus === 'ready' ? 'text-(--success)'
                        : updateStatus === 'available' ? 'text-(--fg)'
                          : 'text-(--fg)'
                  }`}>
                    {updateStatus === 'idle' ? 'Check for updates' : updateStatus === 'checking' ? 'Looking for updates…' : updateStatus === 'up-to-date' ? "You're up to date" : updateStatus === 'available' ? `v${updateVersion} available` : updateStatus === 'ready' ? 'Ready to install' : updateError ?? 'Update failed'}
                  </p>
                  <p className="text-[10px] text-muted-foreground mt-0.5">
                    {updateStatus === 'idle' ? `Currently on v${__APP_VERSION__}` : updateStatus === 'checking' ? 'Please wait…' : updateStatus === 'up-to-date' ? `v${__APP_VERSION__} is the latest` : updateStatus === 'available' ? 'Ready to download' : updateStatus === 'ready' ? `Restart to apply v${updateVersion}` : 'Check your network connection'}
                  </p>
                </>
              )}
            </div>
          </div>

          {/* Action button */}
          <div className="shrink-0">
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
