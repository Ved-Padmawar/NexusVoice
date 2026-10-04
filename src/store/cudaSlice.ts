import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { relaunch } from '@tauri-apps/plugin-process'
import { toast } from 'sonner'
import { COMMANDS } from '../lib/commands'
import { EVENTS } from '../lib/events'
import { extractErrorMessage } from '../lib/errors'
import type { CudaStatus } from '../types'
import type { StateCreator } from 'zustand'
import type { AppState } from './useAppStore'

/** The on-demand CUDA pack, shared by the first-run picker and the About tab. */
export type CudaSlice = {
  /** `null` until first fetched. */
  cuda: CudaStatus | null
  cudaError: string | null
  /** The post-setup CUDA dialog is showing. */
  cudaOfferOpen: boolean
  /** Open the CUDA dialog shortly after the first-run picker closes, if CUDA applies. */
  offerCuda: () => void
  closeCudaOffer: () => void
  refreshCuda: () => Promise<void>
  downloadCuda: () => Promise<void>
  cancelCudaDownload: () => Promise<void>
  removeCuda: () => Promise<void>
  listenForCudaEvents: () => Promise<() => void>
}

/** Lets the dashboard show before the dialog replaces the closed picker. */
const OFFER_DELAY_MS = 1200

export const createCudaSlice: StateCreator<AppState, [], [], CudaSlice> = (set, get) => ({
  cuda: null,
  cudaError: null,
  cudaOfferOpen: false,

  offerCuda: () => {
    setTimeout(() => {
      const cuda = get().cuda
      if (cuda?.supported && cuda.pack === 'absent') set({ cudaOfferOpen: true, cudaError: null })
    }, OFFER_DELAY_MS)
  },

  closeCudaOffer: () => set({ cudaOfferOpen: false }),

  refreshCuda: async () => {
    try {
      set({ cuda: await invoke<CudaStatus>(COMMANDS.GET_CUDA_STATUS) })
    } catch { /* keep the last status */ }
  },

  downloadCuda: async () => {
    const cuda = get().cuda
    if (!cuda) return
    set({ cuda: { ...cuda, downloadProgress: 0 }, cudaError: null })
    try {
      await invoke(COMMANDS.START_CUDA_DOWNLOAD)
    } catch (e) {
      set({ cudaError: extractErrorMessage(e, 'Could not start the download') })
      await get().refreshCuda()
    }
  },

  cancelCudaDownload: async () => {
    try {
      await invoke(COMMANDS.CANCEL_CUDA_DOWNLOAD)
    } catch { /* the cancelled event refreshes */ }
  },

  removeCuda: async () => {
    try {
      await invoke(COMMANDS.REMOVE_CUDA_PACK)
      set({ cudaError: null })
    } catch (e) {
      set({ cudaError: extractErrorMessage(e, 'Could not remove CUDA') })
    }
    await get().refreshCuda()
  },

  listenForCudaEvents: async () => {
    await get().refreshCuda()
    const unlisteners = await Promise.all([
      listen<number>(EVENTS.CUDA_DOWNLOAD_PROGRESS, (e) => {
        const cuda = get().cuda
        if (cuda) set({ cuda: { ...cuda, downloadProgress: e.payload } })
      }),
      listen(EVENTS.CUDA_DOWNLOAD_COMPLETE, () => {
        void get().refreshCuda()
        // The open dialog shows its own Restart.
        if (!get().cudaOfferOpen) toast.success('CUDA is ready. Restart NexusVoice to use it.', {
          action: { label: 'Restart', onClick: () => { void relaunch() } },
        })
      }),
      listen(EVENTS.CUDA_DOWNLOAD_CANCELLED, () => { void get().refreshCuda() }),
      listen<string>(EVENTS.CUDA_DOWNLOAD_ERROR, (e) => {
        set({ cudaError: e.payload || 'Download failed' })
        void get().refreshCuda()
      }),
    ])
    return () => unlisteners.forEach(fn => fn())
  },
})
