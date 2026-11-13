import { create } from 'zustand'
import { convertFileSrc } from '@tauri-apps/api/core'

type Playback = {
  /** The entry whose audio is loaded; `null` when nothing is. */
  id: number | null
  playing: boolean
  /** Seconds. */
  position: number
  /** Seconds; `null` until the file says. */
  duration: number | null
}

const IDLE: Playback = { id: null, playing: false, position: 0, duration: null }

/** One player for the whole page: starting an entry stops the one before it. */
export const usePlayback = create<Playback>(() => IDLE)

let audio: HTMLAudioElement | null = null
let frame = 0

function element() {
  if (audio) return audio
  const el = new Audio()
  el.addEventListener('ended', () => { cancelAnimationFrame(frame); usePlayback.setState({ ...IDLE }) })
  el.addEventListener('pause', () => { cancelAnimationFrame(frame); usePlayback.setState({ playing: false, position: el.currentTime }) })
  el.addEventListener('play', () => { usePlayback.setState({ playing: true }); tick() })
  el.addEventListener('loadedmetadata', () => {
    if (Number.isFinite(el.duration)) usePlayback.setState({ duration: el.duration })
  })
  el.addEventListener('error', () => { cancelAnimationFrame(frame); usePlayback.setState({ ...IDLE }) })
  audio = el
  return el
}

/** Every frame while playing, so the seek line moves smoothly. */
function tick() {
  usePlayback.setState({ position: element().currentTime })
  frame = requestAnimationFrame(tick)
}

export function toggle(id: number, path: string) {
  const el = element()
  if (usePlayback.getState().id === id) {
    if (el.paused) void el.play(); else el.pause()
    return
  }
  el.pause()
  el.src = convertFileSrc(path)
  usePlayback.setState({ id, playing: false, position: 0, duration: null })
  void el.play()
}

/** Jump to `fraction` (0 to 1) of entry `id`'s audio, starting it when it isn't loaded. */
export function seek(id: number, path: string, fraction: number, fallbackDuration: number | null) {
  if (usePlayback.getState().id !== id) toggle(id, path)
  const el = element()
  const duration = Number.isFinite(el.duration) ? el.duration : fallbackDuration
  if (duration == null) return
  el.currentTime = Math.min(Math.max(fraction, 0), 1) * duration
  usePlayback.setState({ position: el.currentTime })
}

/** Stop if entry `id` is the one playing, e.g. when its audio is deleted. */
export function stopIfPlaying(id: number) {
  if (usePlayback.getState().id !== id) return
  audio?.pause()
  audio?.removeAttribute('src')
  usePlayback.setState({ ...IDLE })
}
