import { useEffect, useRef, type CSSProperties, type KeyboardEvent, type PointerEvent } from 'react'
import { PauseIcon, PlayIcon } from '@phosphor-icons/react'
import { fmtClock as mmss } from '../../shared/utils'
import { seek, stopIfPlaying, toggle, usePlayback } from './playback'

const KEY_STEP = 0.05

/** Play, pause and seek an entry's saved audio. The seek line's thumb shows on
 *  hover and while the entry is loaded. */
export function AudioPlayer({ id, path, seconds }: { id: number; path: string; seconds: number | null }) {
  const mine = usePlayback(s => s.id === id)
  const playing = usePlayback(s => s.id === id && s.playing)
  const position = usePlayback(s => (s.id === id ? s.position : 0))
  const known = usePlayback(s => (s.id === id ? s.duration : null))
  const total = known ?? seconds
  const fraction = mine && total ? Math.min(position / total, 1) : 0
  const track = useRef<HTMLSpanElement>(null)

  // The audio goes with the row: deleting an entry or aging out its file stops it.
  useEffect(() => () => stopIfPlaying(id), [id, path])

  const seekTo = (e: PointerEvent) => {
    const box = track.current?.getBoundingClientRect()
    if (box && box.width > 0) seek(id, path, (e.clientX - box.left) / box.width, seconds)
  }
  const onKeyDown = (e: KeyboardEvent) => {
    const step = e.key === 'ArrowRight' ? KEY_STEP : e.key === 'ArrowLeft' ? -KEY_STEP : 0
    if (step) { e.preventDefault(); seek(id, path, fraction + step, seconds) }
  }

  return (
    <span className="player inline-flex items-center gap-2" data-state={mine ? (playing ? 'playing' : 'paused') : 'idle'} style={{ '--p': fraction } as CSSProperties}>
      <button
        type="button"
        aria-label={playing ? 'Pause' : 'Play'}
        onClick={() => toggle(id, path)}
        className="grid size-6 shrink-0 cursor-pointer place-items-center rounded-md bg-surface text-fg ring-1 ring-line-soft transition-[box-shadow,color] ring-inset hover:ring-line"
      >
        {playing ? <PauseIcon size={10} weight="fill" /> : <PlayIcon size={10} weight="fill" />}
      </button>
      <span
        ref={track}
        role="slider"
        tabIndex={0}
        aria-label="Seek"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(fraction * 100)}
        className="track w-36"
        onPointerDown={(e) => { e.currentTarget.setPointerCapture(e.pointerId); seekTo(e) }}
        onPointerMove={(e) => { if (e.currentTarget.hasPointerCapture(e.pointerId)) seekTo(e) }}
        onKeyDown={onKeyDown}
      >
        <span className="fill" />
        <span className="thumb" />
      </span>
      <span className="w-[68px] text-xs text-muted tabular-nums">
        {mine && total ? `${mmss(position)} / ${mmss(total)}` : total ? mmss(total) : ''}
      </span>
    </span>
  )
}
