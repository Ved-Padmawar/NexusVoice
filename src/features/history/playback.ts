import { create } from 'zustand'
import { convertFileSrc } from '@tauri-apps/api/core'
import { toast } from 'sonner'

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

type Track = { id: number; path: string; audio: HTMLAudioElement; request: number }
let current: Track | null = null
let frame = 0

function release() {
  const track = current
  current = null
  cancelAnimationFrame(frame)
  track?.audio.pause()
  track?.audio.removeAttribute('src')
  usePlayback.setState(IDLE)
}

function fail(track: Track) {
  if (current !== track) return
  release()
  toast.error('Could not play the saved audio.')
}

function play(track: Track) {
  const request = ++track.request
  void track.audio.play().catch(() => {
    if (track.request === request) fail(track)
  })
}

function load(id: number, path: string) {
  release()
  const audio = new Audio()
  const track = { id, path, audio, request: 0 }
  const tick = () => {
    if (current !== track) return
    usePlayback.setState({ position: audio.currentTime })
    frame = requestAnimationFrame(tick)
  }
  audio.addEventListener('ended', () => { if (current === track) release() })
  audio.addEventListener('pause', () => {
    if (current !== track || !audio.paused) return
    cancelAnimationFrame(frame)
    usePlayback.setState({ playing: false, position: audio.currentTime })
  })
  audio.addEventListener('play', () => {
    if (current !== track || audio.paused) return
    cancelAnimationFrame(frame)
    usePlayback.setState({ playing: true })
    tick()
  })
  audio.addEventListener('loadedmetadata', () => {
    if (current === track && Number.isFinite(audio.duration)) usePlayback.setState({ duration: audio.duration })
  })
  audio.addEventListener('error', () => fail(track))
  current = track
  usePlayback.setState({ ...IDLE, id })
  audio.src = convertFileSrc(path)
  return track
}

export function toggle(id: number, path: string) {
  if (current?.id !== id || current.path !== path) {
    play(load(id, path))
    return
  }
  if (current.audio.paused) play(current)
  else {
    current.request += 1
    current.audio.pause()
  }
}

/** Jump to `fraction` (0 to 1) of entry `id`'s audio, starting it when it isn't loaded. */
export function seek(id: number, path: string, fraction: number, fallbackDuration: number | null) {
  if (current?.id !== id || current.path !== path) toggle(id, path)
  const audio = current?.audio
  if (!audio) return
  const duration = Number.isFinite(audio.duration) ? audio.duration : fallbackDuration
  if (duration == null) return
  audio.currentTime = Math.min(Math.max(fraction, 0), 1) * duration
  usePlayback.setState({ position: audio.currentTime })
}

/** Stop if entry `id` is the one playing, e.g. when its audio is deleted. */
export function stopIfPlaying(id: number) {
  if (current?.id === id) release()
}
