import { useEffect, useState } from 'react'
import { createPortal } from 'react-dom'
import { relaunch } from '@tauri-apps/plugin-process'
import { WarningCircleIcon, CheckCircleIcon, DownloadSimpleIcon, HardDrivesIcon, ArrowsClockwiseIcon, ArrowCounterClockwiseIcon, ShieldCheckIcon, XIcon, LightningIcon } from '@phosphor-icons/react'
import { Button } from '../../shared/ui/Button'
import { commands } from '../../shared/bindings'
import { useAppStore } from '../../app/store/useAppStore'

const ICON_BUTTON =
  'flex items-center justify-center w-6 h-6 shrink-0 rounded-sm text-muted bg-transparent border-none cursor-pointer transition-colors duration-120 hover:text-danger hover:bg-danger-soft'

/** Offers the CUDA pack on NVIDIA GPUs, and a retry when replacing an
 * unverified pack failed. */
export function CudaOfferModal() {
  const cuda = useAppStore(s => s.cuda)
  const error = useAppStore(s => s.cudaError)
  const downloadCuda = useAppStore(s => s.downloadCuda)
  const cancelDownload = useAppStore(s => s.cancelCudaDownload)
  const close = useAppStore(s => s.closeCudaOffer)
  const [gpuName, setGpuName] = useState<string | null>(null)

  useEffect(() => {
    commands.getHardwareProfile().then(p => setGpuName(p.gpuName)).catch(() => {})
  }, [])

  const progress = cuda?.downloadProgress ?? null
  const closeButton = (
    <button type="button" title="Close" onClick={close} className={`absolute top-3 right-3 ${ICON_BUTTON}`}>
      <XIcon size={13} weight="bold" />
    </button>
  )

  const body =
    cuda?.pack === 'unverified' && !error ? (
      <>
        {closeButton}
        <div className="flex items-center gap-3 pr-7">
          <Badge inline><ShieldCheckIcon size={17} weight="bold" /></Badge>
          <Title>Couldn’t update CUDA</Title>
        </div>
        <p className="mt-3 text-[12px] leading-normal text-muted">
          NexusVoice now verifies the CUDA pack before using it, to make sure it’s the genuine, untampered download.
          Your current copy was installed before this check existed, so NexusVoice tried to download a verified copy,
          but the download didn’t finish.
        </p>
        <Text>Retry to keep transcribing on your NVIDIA GPU.</Text>
        <div className="mt-3 px-2.5 py-2 rounded-md bg-surface border border-line text-[11px] text-fg-2">
          Until then, NexusVoice keeps working on Vulkan, just a bit slower.
        </div>
        <PackFacts />
        {progress !== null ? (
          <Transfer progress={progress} verifying={cuda.verifying} onCancel={cancelDownload} />
        ) : (
          <Actions>
            <Button variant="outline" onClick={close}>Not now</Button>
            <Button variant="tinted" onClick={downloadCuda}><ArrowsClockwiseIcon size={11} weight="bold" />Retry</Button>
          </Actions>
        )}
      </>
    ) : progress !== null ? (
      <>
        <Badge><LightningIcon size={17} weight="bold" /></Badge>
        <Title>Downloading CUDA</Title>
        <Text>You can keep using NexusVoice on Vulkan while this finishes.</Text>
        <Transfer progress={progress} verifying={cuda?.verifying ?? false} onCancel={cancelDownload} />
      </>
    ) : error ? (
      <>
        {closeButton}
        <Badge tone="danger"><WarningCircleIcon size={17} weight="bold" /></Badge>
        <Title>CUDA download failed</Title>
        <Text>{error}. NexusVoice keeps working on Vulkan, and you can retry later from Settings → About.</Text>
        <Actions>
          <Button variant="outline" onClick={close}>Close</Button>
          <Button variant="tinted" onClick={downloadCuda}><ArrowsClockwiseIcon size={11} weight="bold" />Retry</Button>
        </Actions>
      </>
    ) : cuda?.pack === 'pendingInstall' ? (
      <>
        {closeButton}
        <Badge tone="success"><CheckCircleIcon size={17} weight="bold" /></Badge>
        <Title>CUDA is ready</Title>
        <Text>Restart NexusVoice to start transcribing on CUDA.</Text>
        <Actions>
          <Button variant="outline" onClick={close}>Later</Button>
          <Button variant="tinted" onClick={() => relaunch()}><ArrowCounterClockwiseIcon size={11} weight="bold" />Restart now</Button>
        </Actions>
      </>
    ) : (
      <>
        {closeButton}
        <Badge><LightningIcon size={17} weight="bold" /></Badge>
        <Title>Your NVIDIA GPU supports CUDA</Title>
        <Text>
          CUDA makes transcription faster on {gpuName ?? 'your GPU'}. You can add it now or later from Settings → About.
        </Text>
        <PackFacts />
        <Actions>
          <Button variant="outline" onClick={close}>Not now</Button>
          <Button variant="tinted" onClick={downloadCuda}><DownloadSimpleIcon size={11} weight="bold" />Download CUDA</Button>
        </Actions>
      </>
    )

  return createPortal(
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-[2px]">
      <div className="relative w-95 p-5 bg-panel border border-line rounded-xl shadow-(--shadow-lg) transition-[opacity,scale,translate] duration-220 ease-out starting:translate-y-2 starting:scale-96 starting:opacity-0">
        {body}
      </div>
    </div>,
    document.body,
  )
}

function Badge({ tone, inline, children }: { tone?: 'success' | 'danger'; inline?: boolean; children: React.ReactNode }) {
  const colors = tone === 'success' ? 'bg-success-soft text-success'
    : tone === 'danger' ? 'bg-danger-soft text-danger'
      : 'bg-accent-soft text-accent'
  const placement = inline ? 'shrink-0' : 'mb-3.5'
  return <div className={`grid place-items-center w-9 h-9 rounded-lg ${placement} ${colors}`}>{children}</div>
}

function Title({ children }: { children: React.ReactNode }) {
  return <h4 className="text-[15px] font-bold tracking-tight text-fg">{children}</h4>
}

function Text({ children }: { children: React.ReactNode }) {
  return <p className="mt-1.5 text-[12px] leading-normal text-muted">{children}</p>
}

/** Status, percent and cancel above a full-width bar, in the height of a button row. */
function Transfer({ progress, verifying, onCancel }: { progress: number; verifying: boolean; onCancel: () => void }) {
  return (
    <div className="flex flex-col justify-between h-8 mt-4.5">
      <div className="flex items-center gap-2 h-5 text-[11px] leading-none">
        <span className="flex-1 font-medium text-fg-2">{verifying ? 'Verifying…' : 'Downloading'}</span>
        <span className="font-semibold text-accent tabular-nums">{progress}%</span>
        <button type="button" title="Cancel" onClick={onCancel} className={`${ICON_BUTTON} w-5 h-5 -mr-1`}>
          <XIcon size={12} weight="bold" />
        </button>
      </div>
      <div className="h-1 mb-0.5 rounded-full bg-line overflow-hidden">
        <div
          className="h-full rounded-full bg-accent transition-[width] duration-300 ease-linear"
          style={{ width: `${progress}%` }}
        />
      </div>
    </div>
  )
}

function PackFacts() {
  return (
    <div className="flex gap-3.5 mt-3.5 text-[11px] text-fg-2">
      <span className="flex items-center gap-1.25"><HardDrivesIcon size={11} className="text-muted" />About 500 MB download</span>
      <span className="flex items-center gap-1.25"><ArrowCounterClockwiseIcon size={11} className="text-muted" />Applies after a restart</span>
    </div>
  )
}

function Actions({ children }: { children: React.ReactNode }) {
  return <div className="flex gap-2 mt-4.5 *:flex-1">{children}</div>
}
