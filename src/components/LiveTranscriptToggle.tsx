import { emit } from '@tauri-apps/api/event'
import { Captions } from 'lucide-react'
import { EVENTS } from '../lib/events'
import { useAppStore } from '../store/useAppStore'
import { Section } from './Section'
import { Switch } from './Switch'

/** Expand the pill into a live transcript card as you speak. */
export function LiveTranscriptToggle() {
  const liveTranscript = useAppStore((s) => s.liveTranscript)
  const setLiveTranscript = useAppStore((s) => s.setLiveTranscript)

  const toggle = () => {
    const next = !liveTranscript
    setLiveTranscript(next)
    void emit(EVENTS.PILL_LIVE_TRANSCRIPT_CHANGED, next)
  }

  return (
    <Section
      title="Live transcript in the pill"
      Icon={Captions}
      description="The pill grows into a card and fills in as you speak. A large model on modest hardware may lag behind your voice."
      action={<Switch checked={liveTranscript} onChange={toggle} label="Live transcript in the pill" />}
    />
  )
}
