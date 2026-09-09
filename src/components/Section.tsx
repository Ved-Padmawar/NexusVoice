import type { ComponentType, ReactNode, SVGProps } from 'react'

/**
 * A settings block. The header carries the name and the plain-language
 * explanation; `action` sits on the same line for a single control (a switch,
 * a button), and `children` becomes the body below a rule when there is more.
 */
export function Section({
  title,
  Icon,
  description,
  action,
  children,
  bodyClassName,
  stackDescription,
}: {
  title: string
  /** Names the section at a glance, so a scrolled page is scannable. */
  Icon?: ComponentType<SVGProps<SVGSVGElement> & { size?: number | string; strokeWidth?: number | string }>
  description?: ReactNode
  /** Put the description on its own line. Worth it when it would wrap beside
   *  the title anyway — two ragged lines next to a heading read worse than one
   *  clean block under it. */
  stackDescription?: boolean
  action?: ReactNode
  children?: ReactNode
  bodyClassName?: string
}) {
  return (
    <section className="panel">
      <header className="flex items-center gap-x-3 gap-y-0.5 px-4 py-2.5">
        {Icon && (
          <span className="grid size-6 shrink-0 place-items-center rounded-(--r-sm) bg-(--accent-soft) text-(--on-soft)">
            <Icon size={13} strokeWidth={1.9} />
          </span>
        )}
        <div className={
          stackDescription
            ? 'flex min-w-0 flex-1 flex-col gap-0.5'
            : 'flex min-w-0 flex-1 flex-wrap items-baseline gap-x-2.5 gap-y-0.5'
        }>
          <h2 className="m-0 shrink-0 text-[12.5px] font-semibold tracking-[-0.005em] text-(--fg)">
            {title}
          </h2>
          {description && (
            <p className="m-0 min-w-0 flex-1 text-[11.5px] leading-[1.5] text-(--muted)">
              {description}
            </p>
          )}
        </div>
        {action && <div className="shrink-0">{action}</div>}
      </header>
      {children && (
        <>
          <div className="rule h-px" />
          <div className={bodyClassName ?? 'p-4'}>{children}</div>
        </>
      )}
    </section>
  )
}
