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
  /** The retry for an unverified pack was shown this session. */
  cudaRetryShown: boolean
  /** Open the CUDA dialog: once to offer an absent pack, once a session to
   * retry an unverified one. */
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
  cudaRetryShown: false,

  offerCuda: () => {
    clearTimeout(offerTimer)
    // Re-checked when the timer fires: the picker or a download may have moved on.
    const due = () => {
      const { cuda, modelChosen, cudaOfferSeen, cudaRetryShown, cudaOfferOpen } = get()
      if (!modelChosen || cudaOfferOpen || cuda?.supported !== true || cuda.downloadProgress !== null) return false
      if (cuda.pack === 'absent') return !cudaOfferSeen
      return cuda.pack === 'unverified' && !cudaRetryShown
    }
    if (!due()) return
    offerTimer = setTimeout(() => {
      // Seen once shown, however it is left (Restart relaunches without closing).
      if (due()) set({ cudaOfferOpen: true, cudaOfferSeen: true, cudaRetryShown: true, cudaError: null })
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

  // The UI switches on the backend's first progress event, not here: a start
  // that is refused never flashes a progress bar.
  downloadCuda: async () => {
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

  // Listen before reading, so no event from a download started at launch is lost.
  listenForCudaEvents: async () => {
    const unlisteners = await Promise.all([
      events.cudaDownloadProgress.listen((e) => {
        const cuda = get().cuda
        // A running download supersedes the last failure.
        if (cuda) set({ cuda: { ...cuda, downloadProgress: e.payload, verifying: e.payload === 0 ? false : cuda.verifying }, cudaError: null })
      }),
      events.cudaDownloadVerifying.listen(() => {
        const cuda = get().cuda
        if (cuda) set({ cuda: { ...cuda, verifying: true } })
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
        void get().refreshCuda().then(() => get().offerCuda())
      }),
    ])
    await get().refreshCuda()
    return () => unlisteners.forEach(fn => fn())
  },
})
