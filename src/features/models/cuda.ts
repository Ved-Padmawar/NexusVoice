import { useQuery } from '@tanstack/react-query'
import { relaunch } from '@tauri-apps/plugin-process'
import { toast } from 'sonner'
import { commands, events } from '../../shared/bindings'
import { extractErrorMessage } from '../../shared/errors'
import { live, patchLive, readFresh } from '../../app/queryClient'
import { useAppStore } from '../../app/store/useAppStore'
import { queryKeys } from '../../shared/queryKeys'
import type { CudaStatus } from '../../shared/types'

const cudaOptions = live(queryKeys.cuda, () => commands.getCudaStatus())

export function useCuda() {
  return useQuery(cudaOptions)
}

export async function refreshCuda() {
  try {
    const cuda = await readFresh(cudaOptions)
    // CUDA added from About counts too: removing it later must not re-offer it.
    if (cuda.pack !== 'absent') useAppStore.setState({ cudaOfferSeen: true })
  } catch { /* keep the last status */ }
}

const setCudaError = (cudaError: string | null) => useAppStore.setState({ cudaError })

// The UI switches on the backend's first progress event, not here: a start
// that is refused never flashes a progress bar.
export async function downloadCuda() {
  try {
    await commands.startCudaDownload()
  } catch (e) {
    setCudaError(extractErrorMessage(e, 'Could not start the download'))
    await refreshCuda()
  }
}

export async function cancelCudaDownload() {
  try {
    await commands.cancelCudaDownload()
  } catch { /* the cancelled event refreshes */ }
}

export async function removeCuda() {
  try {
    await commands.removeCudaPack()
    setCudaError(null)
  } catch (e) {
    setCudaError(extractErrorMessage(e, 'Could not remove CUDA'))
  }
  await refreshCuda()
}

const patchCuda = (patch: (cuda: CudaStatus) => CudaStatus) => patchLive(cudaOptions, patch)

// Listen before reading, so no event from a download started at launch is lost.
export async function listenForCudaEvents(): Promise<() => void> {
  const unlisteners = await Promise.all([
    events.cudaDownloadProgress.listen((e) => {
      // A running download supersedes the last failure.
      setCudaError(null)
      patchCuda(cuda => ({ ...cuda, downloadProgress: e.payload, verifying: e.payload === 0 ? false : cuda.verifying }))
    }),
    events.cudaDownloadVerifying.listen(() => patchCuda(cuda => ({ ...cuda, verifying: true }))),
    events.cudaDownloadComplete.listen(() => {
      void refreshCuda()
      // The open dialog shows its own Restart.
      if (!useAppStore.getState().cudaOfferOpen) toast.success('CUDA is ready. Restart NexusVoice to use it.', {
        action: { label: 'Restart', onClick: () => { void relaunch() } },
      })
    }),
    events.cudaDownloadCancelled.listen(() => { void refreshCuda() }),
    events.cudaDownloadError.listen((e) => {
      setCudaError(e.payload || 'Download failed')
      void refreshCuda().then(() => useAppStore.getState().offerCuda())
    }),
  ])
  await refreshCuda()
  return () => unlisteners.forEach(fn => fn())
}
