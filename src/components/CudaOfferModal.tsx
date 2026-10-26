import { useEffect, useState } from 'react'
import { createPortal } from 'react-dom'
import { relaunch } from '@tauri-apps/plugin-process'
import { motion } from 'framer-motion'
import { AlertCircle, CheckCircle2, Download, HardDrive, RefreshCw, RotateCcw, ShieldCheck, X, Zap } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { commands } from '../bindings'
import { useAppStore } from '../store/useAppStore'

const ICON_BUTTON =
  'flex items-center justify-center w-6 h-6 shrink-0 rounded-(--r-sm) text-muted-foreground bg-transparent border-none cursor-pointer transition-colors duration-(--t-fast) hover:text-destructive hover:bg-(--danger-soft)'

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
      <X size={13} strokeWidth={2} />
    </button>
  )

  const body =
    cuda?.pack === 'unverified' && !error ? (
      <>
        {closeButton}
        <div className="flex items-center gap-3 pr-7">
          <Badge inline><ShieldCheck size={17} strokeWidth={2} /></Badge>
          <Title>Couldn’t update CUDA</Title>
        </div>
        <p className="mt-3 text-[12px] leading-normal text-muted-foreground">
          NexusVoice now verifies the CUDA pack before using it, to make sure it’s the genuine, untampered download.
          Your current copy was installed before this check existed, so NexusVoice tried to download a verified copy,
          but the download didn’t finish.
        </p>
        <Text>Retry to keep transcribing on your NVIDIA GPU.</Text>
        <div className="mt-3 px-2.5 py-2 rounded-(--r-md) bg-(--surface) border border-(--border) text-[11px] text-(--fg-2)">
          Until then, NexusVoice keeps working on Vulkan, just a bit slower.
        </div>
        <PackFacts />
        {progress !== null ? (
          <Transfer progress={progress} verifying={cuda.verifying} onCancel={cancelDownload} />
        ) : (
          <Actions>
            <Button size="sm" variant="outline" onClick={close}>Not now</Button>
            <Button size="sm" variant="tinted" onClick={downloadCuda}><RefreshCw size={11} strokeWidth={2} />Retry</Button>
          </Actions>
        )}
      </>
    ) : progress !== null ? (
      <>
        <Badge><Zap size={17} strokeWidth={2} /></Badge>
        <Title>Downloading CUDA</Title>
        <Text>You can keep using NexusVoice on Vulkan while this finishes.</Text>
        <Transfer progress={progress} verifying={cuda?.verifying ?? false} onCancel={cancelDownload} />
      </>
    ) : error ? (
      <>
        {closeButton}
        <Badge tone="danger"><AlertCircle size={17} strokeWidth={2} /></Badge>
        <Title>CUDA download failed</Title>
        <Text>{error}. NexusVoice keeps working on Vulkan, and you can retry later from Settings → About.</Text>
        <Actions>
          <Button size="sm" variant="outline" onClick={close}>Close</Button>
          <Button size="sm" variant="tinted" onClick={downloadCuda}><RefreshCw size={11} strokeWidth={2} />Retry</Button>
        </Actions>
      </>
    ) : cuda?.pack === 'pendingInstall' ? (
      <>
        {closeButton}
        <Badge tone="success"><CheckCircle2 size={17} strokeWidth={2} /></Badge>
        <Title>CUDA is ready</Title>
        <Text>Restart NexusVoice to start transcribing on CUDA.</Text>
        <Actions>
          <Button size="sm" variant="outline" onClick={close}>Later</Button>
          <Button size="sm" variant="tinted" onClick={() => relaunch()}><RotateCcw size={11} strokeWidth={2} />Restart now</Button>
        </Actions>
      </>
    ) : (
      <>
        {closeButton}
        <Badge><Zap size={17} strokeWidth={2} /></Badge>
        <Title>Your NVIDIA GPU supports CUDA</Title>
        <Text>
          CUDA makes transcription faster on {gpuName ?? 'your GPU'}. You can add it now or later from Settings → About.
        </Text>
        <PackFacts />
        <Actions>
          <Button size="sm" variant="outline" onClick={close}>Not now</Button>
          <Button size="sm" variant="tinted" onClick={downloadCuda}><Download size={11} strokeWidth={2} />Download CUDA</Button>
        </Actions>
      </>
    )

  return createPortal(
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-[2px]">
      <motion.div
        initial={{ opacity: 0, scale: 0.96, y: 8 }}
        animate={{ opacity: 1, scale: 1, y: 0 }}
        transition={{ duration: 0.22, ease: 'easeOut' }}
        className="relative w-95 p-5 bg-(--panel) border border-(--border) rounded-(--r-xl) shadow-(--shadow-lg)"
      >
        {body}
      </motion.div>
    </div>,
    document.body,
  )
}

function Badge({ tone, inline, children }: { tone?: 'success' | 'danger'; inline?: boolean; children: React.ReactNode }) {
  const colors = tone === 'success' ? 'bg-(--success-soft) text-(--success)'
    : tone === 'danger' ? 'bg-(--danger-soft) text-destructive'
      : 'bg-(--accent-soft) text-(--accent)'
  const placement = inline ? 'shrink-0' : 'mb-3.5'
  return <div className={`grid place-items-center w-9 h-9 rounded-(--r-lg) ${placement} ${colors}`}>{children}</div>
}

function Title({ children }: { children: React.ReactNode }) {
  return <h4 className="text-[15px] font-bold tracking-tight text-(--fg) m-0">{children}</h4>
}

function Text({ children }: { children: React.ReactNode }) {
  return <p className="mt-1.5 text-[12px] leading-normal text-muted-foreground">{children}</p>
}

/** Status, percent and cancel above a full-width bar, in the height of a button row. */
function Transfer({ progress, verifying, onCancel }: { progress: number; verifying: boolean; onCancel: () => void }) {
  return (
    <div className="flex flex-col justify-between h-8 mt-4.5">
      <div className="flex items-center gap-2 h-5 text-[11px] leading-none">
        <span className="flex-1 font-medium text-(--fg-2)">{verifying ? 'Verifying…' : 'Downloading'}</span>
        <span className="font-semibold text-(--accent) tabular-nums">{progress}%</span>
        <button type="button" title="Cancel" onClick={onCancel} className={`${ICON_BUTTON} w-5 h-5 -mr-1`}>
          <X size={12} strokeWidth={2} />
        </button>
      </div>
      <div className="h-1 mb-0.5 rounded-full bg-(--border) overflow-hidden">
        <div
          className="h-full rounded-full bg-(--accent) transition-[width] duration-300 ease-linear"
          style={{ width: `${progress}%` }}
        />
      </div>
    </div>
  )
}

function PackFacts() {
  return (
    <div className="flex gap-3.5 mt-3.5 text-[11px] text-(--fg-2)">
      <span className="flex items-center gap-1.25"><HardDrive size={11} className="text-muted-foreground" />About 500 MB download</span>
      <span className="flex items-center gap-1.25"><RotateCcw size={11} className="text-muted-foreground" />Applies after a restart</span>
    </div>
  )
}

function Actions({ children }: { children: React.ReactNode }) {
  return <div className="flex gap-2 mt-4.5 *:flex-1">{children}</div>
}
