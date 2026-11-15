import { useEffect } from 'react'
import { relaunchApp } from '../../shared/relaunch'
import { toast } from 'sonner'
import { commands, events } from '../../shared/bindings'
import { extractErrorMessage } from '../../shared/errors'
import { latest, mirrored, useSlice } from '../../app/mirror'
import { useAppStore } from '../../app/store/useAppStore'

/** The CUDA pack and its download; null until Rust has probed the GPU. */
export function useCuda() {
  const cuda = useSlice('cuda', status => status)
  // CUDA added from About counts too: removing it later must not re-offer it.
  const installed = cuda !== null && cuda.pack !== 'absent'
  useEffect(() => {
    if (installed) useAppStore.setState({ cudaOfferSeen: true })
  }, [installed])
  return cuda
}

const setCudaError = (cudaError: string | null) => useAppStore.setState({ cudaError })

// The UI switches on the status Rust reports, not here: a start that is
// refused never flashes a progress bar.
export async function downloadCuda() {
  setCudaError(null)
  try {
    await commands.startCudaDownload()
  } catch (e) {
    setCudaError(extractErrorMessage(e, 'Could not start the download'))
  }
}

export async function cancelCudaDownload() {
  try {
    await commands.cancelCudaDownload()
  } catch { /* the status shows what Rust holds */ }
}

export async function removeCuda() {
  try {
    await commands.removeCudaPack()
    setCudaError(null)
  } catch (e) {
    setCudaError(extractErrorMessage(e, 'Could not remove CUDA'))
  }
}

/** Toast a finished download; a failed one reopens the offer with its error. */
export async function listenForCudaNotices(): Promise<() => void> {
  // A notice from an earlier attempt must not touch a newer download.
  const stillLatest = async (attempt: number, rev: number) => {
    await mirrored('cuda', rev)
    const cuda = latest('cuda')
    return cuda?.attempt === attempt && cuda.downloadProgress === null
  }
  const unlisteners = await Promise.all([
    events.cudaDownloadComplete.listen(async ({ payload: { attempt, after } }) => {
      if (!await stillLatest(attempt, after.rev)) return
      // The open dialog shows its own Restart.
      if (!useAppStore.getState().cudaOfferOpen) toast.success('CUDA is ready. Restart NexusVoice to use it.', {
        action: { label: 'Restart', onClick: () => { void relaunchApp() } },
      })
    }),
    events.cudaDownloadError.listen(async ({ payload: { attempt, message, after } }) => {
      if (!await stillLatest(attempt, after.rev)) return
      setCudaError(message || 'Download failed')
      useAppStore.getState().offerCuda()
    }),
  ])
  return () => unlisteners.forEach(fn => fn())
}
