import { useState, useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import {
  AlertCircle, ArrowUpCircle, CheckCircle2, Cpu, Download, FolderOpen, HardDrive,
  MemoryStick, Monitor, RefreshCw, ShieldCheck,
} from 'lucide-react'
import { COMMANDS } from '../../lib/commands'
import { fetchDownloadedModels, formatModelSize } from '../../lib/models'
import type { DownloadedModel, HardwareProfile } from '../../types'
import { useAppStore } from '../../store/useAppStore'
import type { UpdateStatus } from '../../store/updateSlice'
import { Button } from '@/components/ui/button'
import logoUrl from '../../assets/logo.png'

const UPDATE_COPY: Record<UpdateStatus, { title: (v: string | null) => string; detail: (v: string | null) => string }> = {
  idle:         { title: () => 'Check for updates',         detail: () => `You are on v${__APP_VERSION__}` },
  checking:     { title: () => 'Looking for updates…',      detail: () => 'This takes a moment' },
  'up-to-date': { title: () => "You're up to date",         detail: () => `v${__APP_VERSION__} is the latest` },
  available:    { title: (v) => `v${v} is available`,       detail: () => `You are on v${__APP_VERSION__}` },
  downloading:  { title: () => 'Downloading the update',    detail: (v) => `v${v}` },
  ready:        { title: () => 'Ready to install',          detail: (v) => `Restart to finish moving to v${v}` },
  error:        { title: () => 'Update failed',             detail: () => 'Check your network connection and try again' },
}

function UpdatePanel() {
  // Update state lives in the store so this tab and the title-bar chip drive
  // one install rather than each holding a private updater handle.
  const status = useAppStore(s => s.updateStatus)
  const version = useAppStore(s => s.updateVersion)
  const progress = useAppStore(s => s.updateProgress)
  const error = useAppStore(s => s.updateError)
  const checkForUpdate = useAppStore(s => s.checkForUpdate)
  const installUpdate = useAppStore(s => s.installUpdate)
  const restartForUpdate = useAppStore(s => s.restartForUpdate)

  const copy = UPDATE_COPY[status]
  const tone = status === 'up-to-date' || status === 'ready' ? 'success' : status === 'error' ? 'danger' : 'accent'
  const Icon = status === 'error' ? AlertCircle
    : status === 'up-to-date' || status === 'ready' ? CheckCircle2
      : status === 'available' || status === 'downloading' ? Download
        : RefreshCw

  return (
    <section className="nv-card flex flex-col">
      <div className="nv-panel-head"><h3 className="nv-panel-title">Updates</h3></div>
      <div className="flex flex-1 flex-col justify-center gap-4 px-5 pt-4 pb-5">
        <div className="flex items-center gap-3.5">
          <span className={`nv-mark nv-mark--${tone}`}>
            <Icon size={17} strokeWidth={2} className={status === 'checking' ? 'nv-spin' : undefined} />
          </span>
          <div className="min-w-0">
            <p className="text-[14px] font-semibold text-fg">{copy.title(version)}</p>
            <p className="mt-0.5 truncate text-[12.5px] text-muted">
              {status === 'error' && error ? error : copy.detail(version)}
            </p>
          </div>
        </div>

        {status === 'downloading' ? (
          <div className="flex items-center gap-3">
            <span className="nv-progress">
              <span className="nv-progress__fill transition-[width] duration-300" style={{ width: `${progress}%` }} />
            </span>
            <span className="text-[12px] font-semibold tabular-nums text-accent-text">{progress}%</span>
          </div>
        ) : (
          <div>
            {(status === 'idle' || status === 'up-to-date' || status === 'error') && (
              <Button variant="secondary" onClick={checkForUpdate}>
                <RefreshCw />
                {status === 'up-to-date' ? 'Check again' : status === 'error' ? 'Retry' : 'Check now'}
              </Button>
            )}
            {status === 'checking' && (
              <Button variant="secondary" disabled><RefreshCw className="nv-spin" />Checking…</Button>
            )}
            {status === 'available' && (
              <Button onClick={installUpdate}><Download />Download and install</Button>
            )}
            {status === 'ready' && (
              <Button onClick={restartForUpdate}><ArrowUpCircle />Restart now</Button>
            )}
          </div>
        )}
      </div>
    </section>
  )
}

export function AboutTab() {
  const [profile, setProfile] = useState<HardwareProfile | null>(null)
  const [onDisk, setOnDisk] = useState<DownloadedModel[]>([])

  useEffect(() => {
    invoke<HardwareProfile>(COMMANDS.GET_HARDWARE_PROFILE).then(setProfile).catch(() => {})
    void fetchDownloadedModels().then(setOnDisk)
  }, [])

  const modelBytes = onDisk.reduce((acc, m) => acc + m.sizeBytes, 0)

  const specs = [
    {
      Icon: Monitor,
      label: 'Compute',
      value: profile ? `${profile.gpuName}, ${profile.executionProvider.toUpperCase()}` : 'Detecting…',
    },
    {
      Icon: MemoryStick,
      label: 'Memory',
      value: profile ? `${profile.ramGb} GB RAM${profile.vramGb > 0 ? `, ${profile.vramGb} GB VRAM` : ''}` : 'Detecting…',
    },
    {
      Icon: HardDrive,
      label: 'Models on disk',
      value: onDisk.length > 0
        ? `${onDisk.length} ${onDisk.length === 1 ? 'model' : 'models'}, ${formatModelSize(modelBytes)}`
        : 'None downloaded',
    },
  ]

  return (
    <>
      <section className="nv-card nv-about">
        <img src={logoUrl} alt="" className="nv-about__logo" />
        <div className="min-w-0 flex-1">
          <h2 className="nv-about__name">
            NexusVoice
            <span className="nv-badge nv-badge--neutral tabular-nums">v{__APP_VERSION__}</span>
          </h2>
          <p className="mt-1 text-[13px] text-muted">Local speech to text. Audio never leaves this machine.</p>
        </div>
        <div className="flex flex-wrap gap-2">
          <span className="nv-badge nv-badge--neutral h-7! px-2.5!"><Cpu />transcribe-cpp</span>
          <span className="nv-badge nv-badge--success h-7! px-2.5!"><ShieldCheck />100% on-device</span>
        </div>
      </section>

      <div className="nv-grid-2">
        <section className="nv-card flex flex-col">
          <div className="nv-panel-head">
            <h3 className="nv-panel-title">This computer</h3>
            <Button variant="ghost" size="sm" onClick={() => invoke<void>(COMMANDS.OPEN_LOGS_FOLDER)} title="Open logs folder">
              <FolderOpen />
              Logs
            </Button>
          </div>
          <dl className="nv-specs">
            {specs.map(({ Icon, label, value }) => (
              <div key={label}>
                <dt><Icon aria-hidden />{label}</dt>
                <dd title={value}>{value}</dd>
              </div>
            ))}
          </dl>
        </section>

        <UpdatePanel />
      </div>
    </>
  )
}
