import { useEffect } from 'react'
import { useQuery } from '@tanstack/react-query'
import { relaunchApp } from '../../shared/relaunch'
import { toast } from 'sonner'
import { commands, events } from '../../shared/bindings'
import { extractErrorMessage } from '../../shared/errors'
import { caughtUp, reads } from '../../app/sync'
import { useAppStore } from '../../app/store/useAppStore'
import { queryKeys } from '../../shared/queryKeys'

export function useCuda() {
  const query = useQuery({
    queryKey: queryKeys.cuda,
    queryFn: () => commands.getCudaStatus(),
    ...reads('cuda'),
  })
  // CUDA added from About counts too: removing it later must not re-offer it.
  const installed = query.data !== undefined && query.data.pack !== 'absent'
  useEffect(() => {
    if (installed) useAppStore.setState({ cudaOfferSeen: true })
  }, [installed])
  return query
}

const setCudaError = (cudaError: string | null) => useAppStore.setState({ cudaError })

// The UI switches on the status Rust reports, not here: a start that is
// refused never flashes a progress bar.
export async function downloadCuda() {
  try {
    await commands.startCudaDownload()
    setCudaError(null)
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
  const unlisteners = await Promise.all([
    events.cudaDownloadComplete.listen(() => {
      // The open dialog shows its own Restart.
      if (!useAppStore.getState().cudaOfferOpen) toast.success('CUDA is ready. Restart NexusVoice to use it.', {
        action: { label: 'Restart', onClick: () => { void relaunchApp() } },
      })
    }),
    events.cudaDownloadError.listen((e) => {
      setCudaError(e.payload || 'Download failed')
      // The offer checks the status, which must no longer show the download.
      void caughtUp('cuda').then(() => useAppStore.getState().offerCuda())
    }),
  ])
  return () => unlisteners.forEach(fn => fn())
}
