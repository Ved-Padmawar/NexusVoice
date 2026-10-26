import { relaunch } from '@tauri-apps/plugin-process'
import { toast } from 'sonner'
import { commands, events } from '../bindings'
import { extractErrorMessage } from '../lib/errors'
import type { CudaStatus } from '../types'
import type { StateCreator } from 'zustand'
import type { AppState } from './useAppStore'

/** The on-demand CUDA pack, shared by the CUDA dialog and the About tab. */
export type CudaSlice = {
  /** `null` until first fetched. */
  cuda: CudaStatus | null
  cudaError: string | null
  cudaOfferOpen: boolean
  /** The dialog was shown or CUDA was installed; persisted so it never returns. */
  cudaOfferSeen: boolean
  /** Open the CUDA dialog once, with a model chosen and CUDA available but absent. */
  offerCuda: () => void
  closeCudaOffer: () => void
  refreshCuda: () => Promise<void>
  downloadCuda: () => Promise<void>
  cancelCudaDownload: () => Promise<void>
  removeCuda: () => Promise<void>
  listenForCudaEvents: () => Promise<() => void>
}

/** Lets the dashboard show before the dialog opens. */
const OFFER_DELAY_MS = 1200

let offerTimer: ReturnType<typeof setTimeout> | undefined

export const createCudaSlice: StateCreator<AppState, [], [], CudaSlice> = (set, get) => ({
  cuda: null,
  cudaError: null,
  cudaOfferOpen: false,
  cudaOfferSeen: false,

  offerCuda: () => {
    clearTimeout(offerTimer)
    // Re-checked when the timer fires: the picker or a download may have moved on.
    const due = () => {
      const { cuda, modelChosen, cudaOfferSeen, cudaOfferOpen } = get()
      return modelChosen && !cudaOfferSeen && !cudaOfferOpen
        && cuda?.supported === true && cuda.pack === 'absent' && cuda.downloadProgress === null
    }
    if (!due()) return
    offerTimer = setTimeout(() => {
      // Seen once shown, however it is left (Restart relaunches without closing).
      if (due()) set({ cudaOfferOpen: true, cudaOfferSeen: true, cudaError: null })
    }, OFFER_DELAY_MS)
  },

  closeCudaOffer: () => set({ cudaOfferOpen: false }),

  refreshCuda: async () => {
    try {
      const cuda = await commands.getCudaStatus()
      // CUDA added from About counts too: removing it later must not re-offer it.
      set(cuda.pack === 'absent' ? { cuda } : { cuda, cudaOfferSeen: true })
    } catch { /* keep the last status */ }
  },

  downloadCuda: async () => {
    const cuda = get().cuda
    if (!cuda) return
    set({ cuda: { ...cuda, downloadProgress: 0 }, cudaError: null })
    try {
      await commands.startCudaDownload()
    } catch (e) {
      set({ cudaError: extractErrorMessage(e, 'Could not start the download') })
      await get().refreshCuda()
    }
  },

  cancelCudaDownload: async () => {
    try {
      await commands.cancelCudaDownload()
    } catch { /* the cancelled event refreshes */ }
  },

  removeCuda: async () => {
    try {
      await commands.removeCudaPack()
      set({ cudaError: null })
    } catch (e) {
      set({ cudaError: extractErrorMessage(e, 'Could not remove CUDA') })
    }
    await get().refreshCuda()
  },

  listenForCudaEvents: async () => {
    await get().refreshCuda()
    const unlisteners = await Promise.all([
      events.cudaDownloadProgress.listen((e) => {
        const cuda = get().cuda
        if (cuda) set({ cuda: { ...cuda, downloadProgress: e.payload } })
      }),
      events.cudaDownloadComplete.listen(() => {
        void get().refreshCuda()
        // The open dialog shows its own Restart.
        if (!get().cudaOfferOpen) toast.success('CUDA is ready. Restart NexusVoice to use it.', {
          action: { label: 'Restart', onClick: () => { void relaunch() } },
        })
      }),
      events.cudaDownloadCancelled.listen(() => { void get().refreshCuda() }),
      events.cudaDownloadError.listen((e) => {
        set({ cudaError: e.payload || 'Download failed' })
        void get().refreshCuda()
      }),
    ])
    return () => unlisteners.forEach(fn => fn())
  },
})
