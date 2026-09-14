import { emit } from '@tauri-apps/api/event'
import { EVENTS } from '../lib/events'
import { useAppStore } from '../store/useAppStore'
import { Switch } from './ui/switch'
import { SettingRow } from './page'

/** Expand the pill into a live transcript card as you speak. */
export function LiveTranscriptToggle() {
  const liveTranscript = useAppStore((s) => s.liveTranscript)
  const setLiveTranscript = useAppStore((s) => s.setLiveTranscript)

  const toggle = (next: boolean) => {
    setLiveTranscript(next)
    void emit(EVENTS.PILL_LIVE_TRANSCRIPT_CHANGED, next)
  }

  return (
    <SettingRow
      title="Live transcript in the pill"
      description="The pill opens into a card and fills in as you speak. A large model on modest hardware may lag behind your voice."
      inline
    >
      <Switch checked={liveTranscript} onChange={toggle} label="Live transcript in the pill" />
    </SettingRow>
  )
}
