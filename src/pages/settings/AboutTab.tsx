import { useState, useEffect } from 'react'
import { motion } from 'framer-motion'
import { invoke } from '@tauri-apps/api/core'
import {
  AlertCircle, CheckCircle2, HardDrive, MemoryStick, Mic, Monitor,
  RefreshCw, Download, ArrowUpCircle, Cpu, Shield, RotateCcw, Trash2, Zap, X,
} from 'lucide-react'
import { relaunch } from '@tauri-apps/plugin-process'
import { Button } from '@/components/ui/button'
import { COMMANDS } from '../../lib/commands'
import type { HardwareProfile } from '../../types'
import { useAppStore } from '../../store/useAppStore'
import type { UpdateStatus } from '../../store/updateSlice'

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

type Tone = 'accent' | 'success' | 'danger'

const BADGE: Record<Tone, string> = {
  accent: 'bg-(--accent-soft) text-(--accent)',
  success: 'bg-(--success-soft) text-(--success)',
  danger: 'bg-(--danger-soft) text-destructive',
}
const TITLE: Record<Tone, string> = {
  accent: 'text-(--fg)',
  success: 'text-(--success)',
  danger: 'text-destructive',
}

function Section({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <section>
      <div className="mb-2 ml-1 text-[10px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">
        {label}
      </div>
      {children}
    </section>
  )
}

/** One flat status card: badge, title, then a detail line or a progress line. */
function StatusTile({ icon, tone, title, detail, detailTone, progress, onCancel, action }: {
  icon: React.ReactNode
  tone: Tone
  title: string
  detail?: string
  detailTone?: Tone
  /** Shows a progress line in place of the detail. */
  progress?: number
  onCancel?: () => void
  action?: React.ReactNode
}) {
  return (
    <div className="flex items-center gap-3.5 px-4 py-3.5 rounded-(--r-lg) bg-(--panel) border border-(--border-soft)">
      <div className={`w-8.5 h-8.5 rounded-(--r-md) flex items-center justify-center shrink-0 ${BADGE[tone]}`}>
        {icon}
      </div>
      <div className="flex-1 min-w-0">
        <p className={`text-[12.5px] font-semibold ${TITLE[tone]}`}>{title}</p>
        {progress !== undefined ? (
          <div className="flex items-center gap-2.5 h-6 mt-1">
            <div className="flex-1 h-0.75 rounded-full bg-(--border-soft) overflow-hidden">
              <div
                className="h-full rounded-full bg-(--accent) transition-[width] duration-300 ease-linear"
                style={{ width: `${progress}%` }}
              />
            </div>
            <span className="text-[11px] font-semibold text-(--accent) tabular-nums">{progress}%</span>
            {onCancel && (
              <button
                type="button"
                title="Cancel"
                onClick={onCancel}
                className="flex items-center justify-center w-6 h-6 rounded-(--r-sm) text-muted-foreground bg-transparent border-none cursor-pointer transition-colors duration-(--t-fast) hover:text-destructive hover:bg-(--danger-soft)"
              >
                <X size={13} strokeWidth={2} />
              </button>
            )}
          </div>
        ) : (
          <p className={`text-[11px] mt-0.5 ${detailTone ? TITLE[detailTone] : 'text-muted-foreground'}`}>{detail}</p>
        )}
      </div>
      {action && <div className="shrink-0">{action}</div>}
    </div>
  )
}

/** Download, restart into, or remove the on-demand CUDA pack. NVIDIA only. */
function CudaSection({ activeBackend }: { activeBackend: string | null }) {
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
  const icon = <Zap size={15} strokeWidth={2} />
  const restart = (
    <Button size="sm" onClick={() => relaunch()}>
      <RotateCcw size={11} strokeWidth={2} />
      Restart
    </Button>
  )
  const detail = (text: string) => (error ? { detail: error, detailTone: 'danger' as const } : { detail: text })

  return (
    <Section label="GPU acceleration">
      {progress !== null ? (
        <StatusTile icon={icon} tone="accent" title="Downloading CUDA" progress={progress} onCancel={cancelDownload} />
      ) : cuda.pack === 'pendingInstall' ? (
        <StatusTile icon={icon} tone="accent" title="CUDA downloaded" detail="Restart to start using it" action={restart} />
      ) : cuda.pack === 'pendingRemoval' ? (
        <StatusTile icon={icon} tone="accent" title="CUDA removed" detail="Restart to finish removing it" action={restart} />
      ) : cuda.pack === 'installed' ? (
        <StatusTile
          icon={icon}
          tone={active ? 'success' : 'accent'}
          title={active ? 'CUDA active' : 'CUDA installed, not in use'}
          {...detail(active ? 'Transcription runs on CUDA' : 'CUDA could not start, so NexusVoice uses Vulkan')}
          action={
            <Button
              size="sm"
              variant="outline"
              onClick={removeCuda}
              className="hover:text-destructive hover:border-destructive hover:bg-(--danger-soft)"
            >
              <Trash2 size={11} strokeWidth={2} />
              Remove
            </Button>
          }
        />
      ) : (
        <StatusTile
          icon={icon}
          tone="accent"
          title="CUDA acceleration"
          {...detail('Faster on NVIDIA GPUs · about 500 MB download')}
          action={
            <Button size="sm" onClick={downloadCuda}>
              <Download size={11} strokeWidth={2} />
              Download
            </Button>
          }
        />
      )}
    </Section>
  )
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

/** Check, download and restart into an app update. */
function UpdatesSection() {
  // Update state lives in the store so this tab and the sidebar prompt drive
  // one install rather than each holding a private updater handle.
  const status = useAppStore(s => s.updateStatus)
  const version = useAppStore(s => s.updateVersion)
  const progress = useAppStore(s => s.updateProgress)
  const error = useAppStore(s => s.updateError)
  const checkForUpdate = useAppStore(s => s.checkForUpdate)
  const installUpdate = useAppStore(s => s.installUpdate)

  if (status === 'downloading') {
    return (
      <Section label="Updates">
        <StatusTile icon={<Download size={15} strokeWidth={2} />} tone="accent" title={`Downloading v${version}`} progress={progress} />
      </Section>
    )
  }

  const spinner = (size: number) => (
    <motion.span className="flex" animate={{ rotate: 360 }} transition={{ duration: 1, ease: 'linear', repeat: Infinity }}>
      <RefreshCw size={size} strokeWidth={2} />
    </motion.span>
  )
  const view: Record<Exclude<UpdateStatus, 'downloading'>, { tone: Tone; icon: React.ReactNode; title: string; detail: string; action: React.ReactNode }> = {
    idle: {
      tone: 'accent', icon: <RefreshCw size={15} strokeWidth={2} />,
      title: 'Check for updates', detail: `Currently on v${__APP_VERSION__}`,
      action: <Button size="sm" onClick={checkForUpdate}><RefreshCw size={11} strokeWidth={2} />Check</Button>,
    },
    checking: {
      tone: 'accent', icon: spinner(15),
      title: 'Looking for updates…', detail: 'Please wait…',
      action: <Button size="sm" disabled>{spinner(11)}Checking…</Button>,
    },
    'up-to-date': {
      tone: 'success', icon: <CheckCircle2 size={15} strokeWidth={2} />,
      title: "You're up to date", detail: `v${__APP_VERSION__} is the latest`,
      action: <Button size="sm" onClick={checkForUpdate}><RefreshCw size={11} strokeWidth={2} />Check again</Button>,
    },
    available: {
      tone: 'accent', icon: <ArrowUpCircle size={15} strokeWidth={2} />,
      title: `v${version} available`, detail: 'Ready to download',
      action: <Button size="sm" onClick={installUpdate}><Download size={11} strokeWidth={2} />Download</Button>,
    },
    ready: {
      tone: 'success', icon: <CheckCircle2 size={15} strokeWidth={2} />,
      title: 'Ready to install', detail: `Restart to apply v${version}`,
      action: <Button size="sm" onClick={() => relaunch()}><RotateCcw size={11} strokeWidth={2} />Restart</Button>,
    },
    error: {
      tone: 'danger', icon: <AlertCircle size={15} strokeWidth={2} />,
      title: error ?? 'Update failed', detail: 'Check your network connection',
      action: <Button size="sm" onClick={checkForUpdate}><RefreshCw size={11} strokeWidth={2} />Retry</Button>,
    },
  }
  const { action, ...tile } = view[status]

  return (
    <Section label="Updates">
      <StatusTile {...tile} action={action} />
    </Section>
  )
}

export function AboutTab() {
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
            <span className="text-[16px] font-bold tracking-tight text-(--fg)">NexusVoice</span>
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

      <CudaSection activeBackend={profile?.executionProvider ?? null} />
      <UpdatesSection />

    </div>
  )
}
