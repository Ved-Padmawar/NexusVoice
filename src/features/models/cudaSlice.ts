import type { StateCreator } from 'zustand'
import { queryClient } from '../../app/queryClient'
import type { AppState } from '../../app/store/useAppStore'
import { queryKeys } from '../../shared/queryKeys'
import type { CudaStatus } from '../../shared/types'

/** The CUDA dialog, and the last CUDA failure shown by it and the About tab. */
export type CudaSlice = {
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
}

/** Lets the dashboard show before the dialog opens. */
const OFFER_DELAY_MS = 1200

let offerTimer: ReturnType<typeof setTimeout> | undefined

export const createCudaSlice: StateCreator<AppState, [], [], CudaSlice> = (set, get) => ({
  cudaError: null,
  cudaOfferOpen: false,
  cudaOfferSeen: false,
  cudaRetryShown: false,

  offerCuda: () => {
    clearTimeout(offerTimer)
    // Re-checked when the timer fires: the picker or a download may have moved on.
    const due = () => {
      const cuda = queryClient.getQueryData<CudaStatus>(queryKeys.cuda)
      const { modelChosen, cudaOfferSeen, cudaRetryShown, cudaOfferOpen } = get()
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
})
