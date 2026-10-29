import { useState, useEffect, type ReactNode } from 'react'
import {
  ArrowCircleUpIcon, ArrowClockwiseIcon, ArrowsClockwiseIcon, CheckCircleIcon, DownloadSimpleIcon,
  FolderOpenIcon, GraphicsCardIcon, HardDrivesIcon, LightningIcon, MemoryIcon, TrashIcon,
  WarningCircleIcon, XIcon,
} from '@phosphor-icons/react'
import { relaunch } from '@tauri-apps/plugin-process'
import { toast } from 'sonner'
import { Button } from '../../shared/ui/Button'
import { commands } from '../../shared/bindings'
import { extractErrorMessage } from '../../shared/errors'
import { formatModelSize } from '../models/models'
import type { DownloadedModel, HardwareProfile } from '../../shared/types'
import { useAppStore } from '../../app/store/useAppStore'
import logoUrl from '../../assets/logo.png'

type Tone = 'accent' | 'success' | 'danger'

const TILE: Record<Tone, string> = {
  accent: 'bg-accent-soft text-accent-ink',
  success: 'bg-success-soft text-success',
  danger: 'bg-danger-soft text-danger',
}
const TEXT: Record<Tone, string> = {
  accent: 'text-fg',
  success: 'text-success',
  danger: 'text-danger',
}

/** One status row: tile, title, then a detail line or a progress bar. A running
 *  download takes the action column's room, so the bar reaches the edge. */
function StatusCard({ icon, tone, title, titleTone, detail, detailTone, progress, onCancel, action }: {
  icon: ReactNode
  tone: Tone
  title: string
  titleTone?: Tone
  detail?: string
  detailTone?: Tone
  /** Shows a progress bar in place of the detail. */
  progress?: number
  onCancel?: () => void
  action?: ReactNode
}) {
  return (
    <div className="flex h-19 items-center gap-3.5 rounded-2xl bg-panel px-5 ring-1 ring-line-soft ring-inset">
      <span className={`grid size-9 shrink-0 place-items-center rounded-[10px] text-lg ${TILE[tone]}`}>{icon}</span>
      <div className="min-w-0 flex-1">
        <p className={`truncate text-sm font-semibold ${TEXT[titleTone ?? 'accent']}`}>{title}</p>
        {progress !== undefined ? (
          <div className="mt-1 flex items-center gap-2.5">
            <span className="h-1 flex-1 overflow-hidden rounded-full bg-fg/10">
              <span className="block h-full rounded-lg bg-accent transition-[width] duration-300 ease-linear" style={{ width: `${progress}%` }} />
            </span>
            <span className="w-8 text-right text-xs font-semibold text-fg-2 tabular-nums">{progress}%</span>
            {onCancel && (
              <button
                type="button"
                aria-label="Cancel download"
                onClick={onCancel}
                className="-my-1 grid size-7 shrink-0 cursor-pointer place-items-center rounded-lg text-muted hover:bg-danger-soft hover:text-danger"
              >
                <XIcon size={12} weight="bold" />
              </button>
            )}
          </div>
        ) : (
          <p className={`truncate text-xs ${detailTone ? TEXT[detailTone] : 'text-muted'}`}>{detail}</p>
        )}
      </div>
      {progress === undefined && action}
    </div>
  )
}

const Download = ({ onClick }: { onClick: () => void }) => (
  <Button variant="primary" onClick={onClick}><DownloadSimpleIcon size={12} weight="bold" />Download</Button>
)

/** Download, restart into, or remove the on-demand CUDA pack. NVIDIA only. */
function CudaCard({ activeBackend }: { activeBackend: string | null }) {
  const cuda = useAppStore(s => s.cuda)
  const error = useAppStore(s => s.cudaError)
  const refreshCuda = useAppStore(s => s.refreshCuda)
  const downloadCuda = useAppStore(s => s.downloadCuda)
  const cancelDownload = useAppStore(s => s.cancelCudaDownload)
  const removeCuda = useAppStore(s => s.removeCuda)

  useEffect(() => { void refreshCuda() }, [refreshCuda])

  if (!cuda?.supported) return null

  const progress = cuda.downloadProgress
  const active = activeBackend === 'cuda'
  const icon = <LightningIcon weight="duotone" />
  const restart = (
    <Button variant="outline" onClick={() => relaunch()}><ArrowClockwiseIcon size={12} weight="bold" />Restart</Button>
  )
  const detail = (text: string) => (error ? { detail: error, detailTone: 'danger' as const } : { detail: text })

  if (progress !== null) {
    return <StatusCard icon={icon} tone="accent" title={cuda.verifying ? 'Verifying CUDA' : 'Downloading CUDA'} progress={progress} onCancel={cancelDownload} />
  }
  switch (cuda.pack) {
    case 'pendingInstall':
      return <StatusCard icon={icon} tone="accent" title="CUDA downloaded" detail="Restart to start using it" action={restart} />
    case 'pendingRemoval':
      return <StatusCard icon={icon} tone="accent" title="CUDA removed" detail="Restart to finish removing it" action={restart} />
    case 'installed':
      return (
        <StatusCard
          icon={icon}
          tone={active ? 'success' : 'accent'}
          title={active ? 'CUDA active' : 'CUDA installed, not in use'}
          {...detail(active ? 'Transcription runs on CUDA' : 'CUDA could not start, so NexusVoice uses Vulkan')}
          action={<Button variant="danger" onClick={removeCuda}><TrashIcon size={12} weight="bold" />Remove</Button>}
        />
      )
    case 'unverified':
      return (
        <StatusCard
          icon={icon}
          tone="accent"
          title="CUDA needs a fresh download"
          {...detail('Your copy predates verification, so NexusVoice uses Vulkan')}
          action={<Download onClick={downloadCuda} />}
        />
      )
    default:
      return (
        <StatusCard
          icon={icon}
          tone="accent"
          title="CUDA acceleration"
          {...detail('Faster on NVIDIA GPUs, about 500 MB')}
          action={<Download onClick={downloadCuda} />}
        />
      )
  }
}

/** Check, download and restart into an app update. */
function UpdateCard() {
  const status = useAppStore(s => s.updateStatus)
  const version = useAppStore(s => s.updateVersion)
  const progress = useAppStore(s => s.updateProgress)
  const error = useAppStore(s => s.updateError)
  const checkForUpdate = useAppStore(s => s.checkForUpdate)
  const installUpdate = useAppStore(s => s.installUpdate)
  const restartForUpdate = useAppStore(s => s.restartForUpdate)

  const check = (label: string) => (
    <Button variant="outline" onClick={checkForUpdate}><ArrowsClockwiseIcon size={12} weight="bold" />{label}</Button>
  )
  const arrowUp = <ArrowCircleUpIcon weight="bold" />

  switch (status) {
    case 'downloading':
      return <StatusCard icon={arrowUp} tone="accent" title={`Downloading v${version}`} progress={progress} />
    case 'checking':
      return (
        <StatusCard
          icon={<ArrowsClockwiseIcon weight="bold" className="animate-spin" />}
          tone="accent"
          title="Looking for updates…"
          detail="Please wait…"
          action={<Button variant="outline" disabled><ArrowsClockwiseIcon size={12} weight="bold" className="animate-spin" />Checking…</Button>}
        />
      )
    case 'up-to-date':
      return (
        <StatusCard
          icon={<CheckCircleIcon weight="fill" />}
          tone="success"
          title="You’re up to date"
          detail={`v${__APP_VERSION__} is the latest version`}
          action={check('Check again')}
        />
      )
    case 'available':
      return (
        <StatusCard
          icon={arrowUp}
          tone="accent"
          title={`v${version} is available`}
          detail={`You’re on v${__APP_VERSION__}`}
          action={<Button variant="primary" onClick={installUpdate}><DownloadSimpleIcon size={12} weight="bold" />Install</Button>}
        />
      )
    case 'ready':
      return (
        <StatusCard
          icon={arrowUp}
          tone="accent"
          title="Ready to install"
          titleTone="success"
          detail="Restart to finish updating"
          action={<Button variant="outline" onClick={restartForUpdate}><ArrowClockwiseIcon size={12} weight="bold" />Restart</Button>}
        />
      )
    case 'error':
      return (
        <StatusCard
          icon={<WarningCircleIcon weight="bold" />}
          tone="danger"
          title={error ?? 'Update failed'}
          detail="Check your network connection"
          action={check('Retry')}
        />
      )
    default:
      return (
        <StatusCard
          icon={<ArrowsClockwiseIcon weight="bold" />}
          tone="accent"
          title="Check for updates"
          detail={`Currently on v${__APP_VERSION__}`}
          action={check('Check')}
        />
      )
  }
}

/** One cell of the system-info card. */
function Stat({ icon, label, value, first }: { icon: ReactNode; label: string; value: string; first?: boolean }) {
  return (
    <div className={`flex min-w-0 flex-col gap-1 px-5 py-4 ${first ? '' : 'border-l border-line-soft'}`}>
      <dt className="flex items-center gap-1.5 text-xs text-muted">{icon}{label}</dt>
      <dd className="truncate text-sm font-medium text-fg tabular-nums">{value}</dd>
    </div>
  )
}

export function AboutTab() {
  const [profile, setProfile] = useState<HardwareProfile | null>(null)
  const [onDisk, setOnDisk] = useState<DownloadedModel[]>([])

  useEffect(() => {
    commands.getHardwareProfile().then(setProfile).catch(() => {})
    commands.getDownloadedModels().then(setOnDisk).catch(() => {})
  }, [])

  const modelBytes = onDisk.reduce((acc, m) => acc + m.sizeBytes, 0)

  const openLogs = () => {
    commands.openLogsFolder().catch(e => toast.error(extractErrorMessage(e, 'Could not open the logs folder')))
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="relative isolate flex items-center gap-5 overflow-hidden rounded-2xl bg-panel p-6 ring-1 ring-line-soft ring-inset">
        <div className="pointer-events-none absolute inset-0 -z-10 bg-[radial-gradient(60%_140%_at_100%_50%,var(--accent-soft),transparent_70%)]" />
        <img src={logoUrl} alt="" className="size-14 rounded-2xl" />
        <div className="min-w-0 flex-1">
          <div className="flex items-baseline gap-2">
            <span className="text-xl font-bold tracking-tight text-fg">NexusVoice</span>
            <span className="text-sm font-medium text-muted tabular-nums">v{__APP_VERSION__}</span>
          </div>
          <p className="mt-0.5 text-sm text-fg-2">Speech to text that runs on this computer. Your audio never leaves it.</p>
        </div>
        <Button variant="outline" onClick={openLogs} className="text-fg-2 hover:text-fg">
          <FolderOpenIcon size={12} weight="bold" />Open logs
        </Button>
      </div>

      <dl className="grid grid-cols-3 rounded-2xl bg-panel ring-1 ring-line-soft ring-inset">
        <Stat
          first
          icon={<GraphicsCardIcon size={14} />}
          label="Compute"
          value={profile ? `${profile.gpuName} on ${profile.executionProvider.toUpperCase()}` : 'Detecting…'}
        />
        <Stat
          icon={<MemoryIcon size={14} />}
          label="Memory"
          value={profile ? `${profile.ramGb} GB RAM${profile.vramGb > 0 ? `, ${profile.vramGb} GB VRAM` : ''}` : 'Detecting…'}
        />
        <Stat
          icon={<HardDrivesIcon size={14} />}
          label="Models on disk"
          value={onDisk.length > 0 ? `${onDisk.length} model${onDisk.length > 1 ? 's' : ''}, ${formatModelSize(modelBytes)}` : 'None downloaded'}
        />
      </dl>

      <div className="grid grid-cols-2 gap-4 *:only:col-span-2">
        <CudaCard activeBackend={profile?.executionProvider ?? null} />
        <UpdateCard />
      </div>
    </div>
  )
}
