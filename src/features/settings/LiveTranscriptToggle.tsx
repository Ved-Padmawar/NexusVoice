import { ClosedCaptioningIcon, MicrophoneIcon } from '@phosphor-icons/react'
import { Switch } from '../../shared/ui/Switch'
import { pillThemeDef } from '../../pill/pillThemes'
import { updatePill, usePillSettings } from './queries'
import './LiveTranscriptToggle.css'

/** Expand the pill into a live transcript card as you speak. */
export function LiveTranscriptToggle() {
  const { liveTranscript, theme: pillTheme } = usePillSettings()
  const theme = pillThemeDef(pillTheme)

  return (
    <div className="flex flex-col gap-3 rounded-2xl bg-panel p-4 ring-1 ring-line-soft ring-inset">
      <div className="flex items-center gap-3">
        <span className="grid size-8 place-items-center rounded-[10px] bg-accent-2-soft text-base text-accent-2-ink">
          <ClosedCaptioningIcon size={16} weight="duotone" />
        </span>
        <p className="flex-1 text-sm font-semibold text-fg">Live transcript in the pill</p>
        <Switch
          checked={liveTranscript}
          onChange={(on) => { void updatePill({ liveTranscript: on }) }}
          aria-label="Live transcript in the pill"
        />
      </div>
      <p className="text-xs text-pretty text-muted">
        The pill opens into a card that fills in as you speak. A large model on modest hardware may lag behind your voice.
      </p>
      {/* Match redesign's full-size card, scaled inside the clipped stage. */}
      <div aria-hidden="true" className="mt-auto grid h-20 place-items-center overflow-hidden rounded-lg bg-field ring-1 ring-line-soft ring-inset">
        <div className="origin-center scale-[.6]">
          <div
            className="live-transcript-preview"
            data-pill-theme={pillTheme}
            style={{ background: theme.bg, color: theme.brand, '--preview-accent': theme.accent } as React.CSSProperties}
          >
            <span className="live-transcript-preview__tide" />
            <div className="live-transcript-preview__text">
              <p>
                <span>Let’s move the design review to Thursday and loop in the</span>{' '}
                <span className="live-transcript-preview__tentative">platform team before</span>
                <span className="live-transcript-preview__caret" />
              </p>
            </div>
            <div className="live-transcript-preview__strip">
              <MicrophoneIcon size={12} weight="fill" />
              <span className="live-transcript-preview__wave">
                {Array.from({ length: 8 }, (_, i) => <i key={i} />)}
              </span>
              <span className="live-transcript-preview__hint">Hold to speak</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
