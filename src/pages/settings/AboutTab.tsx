import { useState, useEffect, useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'
import {
  HardDrive, MemoryStick, Monitor, RefreshCw, Download, ArrowUpCircle, FolderOpen,
  Cpu, ArrowDownToLine, LifeBuoy,
} from 'lucide-react'
import { relaunch } from '@tauri-apps/plugin-process'
import { COMMANDS } from '../../lib/commands'
import { Section } from '../../components/Section'
import { Spinner } from '../../components/Spinner'
import type { HardwareProfile } from '../../types'
import { useAppStore } from '../../store/useAppStore'
import logoUrl from '../../assets/logo.png'

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

/** One label/value row in the system readout. */
function InfoRow({ Icon, label, value }: { Icon: typeof Monitor; label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-4 border-b border-(--hairline) px-4 py-2.5 last:border-0">
      <span className="flex items-center gap-2.5 text-[12px] text-(--fg-2)">
        <Icon size={13} strokeWidth={1.9} className="shrink-0 text-(--muted)" />
        {label}
      </span>
      <span className="truncate text-right text-[12px] tabular-nums text-(--muted)">{value}</span>
    </div>
  )
}

export function AboutTab() {
  // Update state lives in the store so this tab and the sidebar card drive one
  // install rather than each holding a private updater handle.
  const updateStatus = useAppStore(s => s.updateStatus)
  const updateVersion = useAppStore(s => s.updateVersion)
  const downloadProgress = useAppStore(s => s.updateProgress)
  const updateError = useAppStore(s => s.updateError)
  const checkForUpdate = useAppStore(s => s.checkForUpdate)
  const downloadAndInstall = useAppStore(s => s.installUpdate)
  const [profile, setProfile] = useState<HardwareProfile | null>(null)
  const [onDisk, setOnDisk] = useState<DownloadedModel[]>([])
  const [checking, setChecking] = useState(false)

  const runCheck = useCallback(() => {
    if (checking) return
    setChecking(true)
    // Same minimum-duration trick as the microphone refresh: a cached result
    // returns in a few ms, and a spinner that never paints looks like a glitch.
    Promise.all([
      checkForUpdate().catch(() => {}),
      new Promise(r => setTimeout(r, 650)),
    ]).finally(() => setChecking(false))
  }, [checking, checkForUpdate])

  useEffect(() => {
    invoke<HardwareProfile>(COMMANDS.GET_HARDWARE_PROFILE).then(setProfile).catch(() => {})
    invoke<DownloadedModel[]>(COMMANDS.GET_DOWNLOADED_MODELS).then(setOnDisk).catch(() => {})
  }, [])

  const modelBytes = onDisk.reduce((acc, m) => acc + m.sizeBytes, 0)

  const busy = checking || updateStatus === 'checking'

  const headline =
    busy ? 'Looking for updates'
      : updateStatus === 'idle' ? `Version ${__APP_VERSION__}`
      : updateStatus === 'up-to-date' ? `Version ${__APP_VERSION__} is the latest`
      : updateStatus === 'available' ? `Version ${updateVersion} is available`
      : updateStatus === 'downloading' ? 'Downloading update'
      : updateStatus === 'ready' ? `Version ${updateVersion} is ready`
      : updateError ?? 'Update check failed'

  const detail =
    busy ? 'Checking the update server.'
      : updateStatus === 'up-to-date' ? 'Nothing to do.'
      : updateStatus === 'available' ? `You are on ${__APP_VERSION__}.`
      : updateStatus === 'ready' ? 'Restart to finish installing.'
      : updateStatus === 'error' ? 'Check your network connection and try again.'
      : 'Updates install in place, without losing your settings.'

  return (
    <div className="flex flex-col gap-4">
      <section className="panel flex items-center gap-4 p-5">
        <img src={logoUrl} alt="" className="size-11 shrink-0 rounded-(--r-lg)" />
        <div className="min-w-0 flex-1">
          <h2 className="m-0 text-[17px] font-semibold tracking-[-0.02em] text-(--fg)">NexusVoice</h2>
          <p className="m-0 mt-1 max-w-prose text-[12px] leading-[1.6] text-(--muted)">
            Speech to text that runs on this machine. Your audio is transcribed
            locally and never leaves the computer.
          </p>
        </div>
        <span className="shrink-0 text-[13px] font-medium tabular-nums text-(--fg-2)">
          {__APP_VERSION__}
        </span>
      </section>

      <Section title="System" Icon={Cpu} bodyClassName="">
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
      </Section>

      <Section
        title="Updates"
        Icon={ArrowDownToLine}
        description={detail}
        action={
          <div className="flex items-center gap-2">
            {updateStatus === 'available' && !busy ? (
              <button type="button" onClick={downloadAndInstall} className="btn btn-sm btn-primary">
                <Download size={11} strokeWidth={2} />
                Download update
              </button>
            ) : updateStatus === 'downloading' ? (
              <button type="button" disabled className="btn btn-sm btn-primary">
                <Spinner size={11} />
                Downloading
              </button>
            ) : updateStatus === 'ready' ? (
              <button type="button" onClick={() => relaunch()} className="btn btn-sm btn-primary">
                <ArrowUpCircle size={11} strokeWidth={2} />
                Restart now
              </button>
            ) : (
              /* One element across idle / checking / up-to-date / error, so the
                 control updates in place rather than unmounting mid-spin. */
              <button type="button" onClick={runCheck} disabled={busy} className="btn btn-sm btn-quiet">
                {busy
                  ? <Spinner size={11} />
                  : <RefreshCw size={11} strokeWidth={2} />}
                {busy ? 'Checking'
                  : updateStatus === 'error' ? 'Try again'
                  : updateStatus === 'up-to-date' ? 'Check again'
                  : 'Check for updates'}
              </button>
            )}
          </div>
        }
        bodyClassName={updateStatus === 'downloading' ? 'px-4 py-3.5' : 'px-4 py-3'}
      >
        {updateStatus === 'downloading' ? (
          <div className="flex flex-col gap-2">
            <div className="h-1 overflow-hidden rounded-full bg-(--bg-alt)">
              <div
                className="h-full rounded-full bg-(--accent) transition-[width] duration-300 ease-out"
                style={{ width: `${downloadProgress}%` }}
              />
            </div>
            <div className="flex items-baseline justify-between">
              <span className="text-[11.5px] text-(--fg-2)">{headline}</span>
              <span className="text-[11.5px] font-semibold tabular-nums text-(--on-soft)">
                {downloadProgress}%
              </span>
            </div>
          </div>
        ) : (
          <span
            className={`text-[12px] transition-colors duration-(--t-mid) ${
              busy ? 'text-(--muted)'
                : updateStatus === 'error' ? 'text-(--danger)'
                : updateStatus === 'up-to-date' || updateStatus === 'ready' ? 'text-(--success)'
                : 'text-(--fg-2)'
            }`}
          >
            {headline}
          </span>
        )}
      </Section>

      <Section
        title="Diagnostics"
        Icon={LifeBuoy}
        description="Logs stay on this machine. Open the folder if you need to send one along with a bug report."
        action={
          <button
            type="button"
            onClick={() => invoke<void>(COMMANDS.OPEN_LOGS_FOLDER)}
            className="btn btn-sm btn-quiet"
          >
            <FolderOpen size={12} strokeWidth={1.9} />
            Open logs folder
          </button>
        }
      />
    </div>
  )
}
