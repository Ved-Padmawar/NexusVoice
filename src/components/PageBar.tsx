import type { ReactNode } from 'react'

/**
 * The top of every page: one line.
 *
 * Chrome lives in the strip above and navigation in the dock below, so all a
 * page still owes is what you are looking at. Name and purpose share the row —
 * they are one thought, and stacking them cost ~20px of an 800px window. `nav`
 * and `children` ride the same row, right-aligned behind the name.
 *
 * `--gutter` / `--measure` are the same pair the pages use, so header, content
 * and dock share one column.
 */
export function PageBar({
  title,
  description,
  nav,
  children,
}: {
  /** The page's name. Every page sets it. */
  title?: string
  /** One line on what the page is for, on the same row as the name. */
  description?: string
  /** Navigation within the page. Right-aligned. */
  nav?: ReactNode
  /** Actions on the whole page. Right-aligned, after `nav`. */
  children?: ReactNode
}) {
  return (
    <div className="shrink-0 px-(--gutter) pb-3 pt-4">
      <div className="mx-auto flex w-full max-w-(--measure) items-center gap-3">
        {title && (
          <>
            <span className="titlemark" aria-hidden />
            <h1 className="m-0 shrink-0 text-[21px] font-semibold leading-none tracking-[-0.03em] text-(--fg)">
              {title}
            </h1>
          </>
        )}
        {description && (
          <>
            <span className="rule h-3.5 w-px shrink-0" />
            {/* Not leading-none: truncate clips overflow, and a line box the
                exact height of the font cuts the descenders off a 'g'. */}
            <p className="m-0 min-w-0 truncate text-[12px] leading-[1.5] text-(--muted)">
              {description}
            </p>
          </>
        )}
        {(nav || children) && (
          <div className="ml-auto flex shrink-0 items-center gap-2 pl-3">
            {nav}
            {children}
          </div>
        )}
      </div>
    </div>
  )
}
