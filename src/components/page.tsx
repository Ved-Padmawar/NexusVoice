import { useRef, type ReactNode, type Ref } from 'react'
import { cn } from '@/lib/utils'
import { useStuck } from '../lib/hooks'

export function PageHeader({ title, description, actions }: {
  title: ReactNode
  description?: ReactNode
  actions?: ReactNode
}) {
  return (
    <header className="nv-page-head">
      <div className="min-w-0">
        <h1 className="nv-page-title">{title}</h1>
        {description && <p className="nv-page-desc">{description}</p>}
      </div>
      {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
    </header>
  )
}

/** Sticks to the top of the page scroller; turns to glass once stuck. */
export function StickyBar({ children, className }: { children: ReactNode; className?: string }) {
  const ref = useRef<HTMLDivElement>(null)
  const stuck = useStuck(ref)
  return (
    <div ref={ref} data-stuck={stuck || undefined} className={cn('nv-sticky', className)}>
      {children}
    </div>
  )
}

export function Section({ title, description, actions, children, className }: {
  title: ReactNode
  description?: ReactNode
  actions?: ReactNode
  children: ReactNode
  className?: string
}) {
  return (
    <section className={cn('nv-section', className)}>
      <div className="nv-section__head">
        <div className="min-w-0">
          <h2 className="nv-section__title">{title}</h2>
          {description && <p className="nv-section__desc">{description}</p>}
        </div>
        {actions}
      </div>
      {children}
    </section>
  )
}

export function SettingGroup({ children }: { children: ReactNode }) {
  return <div className="nv-card nv-group">{children}</div>
}

/** Stacks when the card is narrow, unless `inline`. */
export function SettingRow({ title, description, children, field, inline, ref }: {
  title: ReactNode
  description?: ReactNode
  children: ReactNode
  /** Give the control a fixed field width. */
  field?: boolean
  inline?: boolean
  ref?: Ref<HTMLDivElement>
}) {
  return (
    <div ref={ref} className={cn('nv-row', inline && 'nv-row--inline')}>
      <div className="min-w-0">
        <div className="nv-row__title">{title}</div>
        {description && <div className="nv-row__desc">{description}</div>}
      </div>
      <div className={cn('nv-row__control', field && 'nv-row__control--field')}>{children}</div>
    </div>
  )
}
