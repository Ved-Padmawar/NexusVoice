import { ClosedCaptioningIcon, MicrophoneIcon } from '@phosphor-icons/react'
import { Switch } from '../../shared/ui/Switch'
import { SettingRow } from './SettingRow'
import { pillThemeDef } from '../../pill/pillThemes'
import { updatePill, usePillSettings } from './queries'
import './LiveTranscriptToggle.css'

/** Expand the pill into a live transcript card as you speak. */
export function LiveTranscriptToggle() {
  const { liveTranscript, theme: pillTheme } = usePillSettings()
  const theme = pillThemeDef(pillTheme)

  return (
    <div className="flex flex-1 flex-col">
      <SettingRow
        icon={<ClosedCaptioningIcon size={16} weight="duotone" />}
        title="Live transcript in the pill"
        tip="A large model on modest hardware may lag behind your voice."
        description={<p className="text-xs text-pretty text-muted">The pill opens into a card that fills in as you speak.</p>}
        below={
          <div aria-hidden="true" className={`grid min-h-28 flex-1 place-items-center overflow-hidden rounded-lg bg-[light-dark(var(--surface),var(--field))] transition-opacity ${liveTranscript ? '' : 'opacity-50'}`}>
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
        }
      >
        <Switch
          checked={liveTranscript}
          onChange={(on) => { void updatePill({ liveTranscript: on }) }}
          aria-label="Live transcript in the pill"
        />
      </SettingRow>
    </div>
  )
}
