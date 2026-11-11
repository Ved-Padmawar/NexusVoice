import type { ReactNode } from 'react'
import type { RecordingInput } from '../../shared/bindings'
import { useKeptByRecording } from './queries'

/** A recording input's description, swapped for a "waits for the next
 * recording" note on the same line, so nothing below moves. */
export function InputHint({ input, children }: { input: RecordingInput; children: ReactNode }) {
  const kept = useKeptByRecording(input)
  return (
    <p className={`text-xs ${kept ? 'text-accent-ink' : 'text-muted'}`}>
      {kept ? 'Saved. This recording keeps the previous one.' : children}
    </p>
  )
}
