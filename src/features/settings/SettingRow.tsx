import type { ReactNode } from 'react'
import { InfoIcon } from '@phosphor-icons/react'

/** An ⓘ beside a title: the longer explanation, on hover or focus. */
export function InfoTip({ text }: { text: string }) {
  return (
    <span tabIndex={0} aria-label={text} data-tip={text} className="inline-grid size-4 shrink-0 cursor-help place-items-center text-muted transition-colors hover:text-fg">
      <InfoIcon size={14} />
    </span>
  )
}

const TONE = {
  accent: 'bg-accent-soft text-accent-ink',
  accent2: 'bg-accent-2-soft text-accent-2-ink',
}

/** One setting: icon tile, title over a description, its control on the right.
 *  `below` adds a full-width line under the row. */
export function SettingRow({ icon, tone = 'accent2', title, tip, description, below, children }: {
  icon: ReactNode
  tone?: keyof typeof TONE
  title: string
  tip?: string
  description: ReactNode
  below?: ReactNode
  children: ReactNode
}) {
  return (
    <div className="flex flex-1 flex-col justify-center gap-2.5 px-4 py-3">
      <div className="flex items-center gap-3">
        <span className={`grid size-8 shrink-0 place-items-center rounded-[10px] ${TONE[tone]}`}>{icon}</span>
        <div className="min-w-0 flex-1">
          <p className="flex items-center gap-2 text-sm font-semibold text-fg">{title}{tip && <InfoTip text={tip} />}</p>
          {description}
        </div>
        {children}
      </div>
      {below}
    </div>
  )
}

/** Cards of rows share one look: a hairline between rows. */
export function SettingsCard({ children, className = '' }: { children: ReactNode; className?: string }) {
  return (
    <div className={`flex flex-col divide-y divide-line-soft rounded-2xl bg-panel ring-1 ring-line-soft ring-inset ${className}`}>
      {children}
    </div>
  )
}
