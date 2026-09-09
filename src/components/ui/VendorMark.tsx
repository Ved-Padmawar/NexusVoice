import { RASTER, VENDORS, isRaster, type RasterId, type VendorId } from '../../lib/vendors'

/** Renders a vendor's logo. The registry it reads lives in `lib/vendors`. */
export function VendorMark({
  vendor,
  className,
}: {
  vendor: VendorId | RasterId | 'custom'
  className?: string
}) {
  if (vendor === 'custom') {
    return (
      // --border measures 1.3:1 against the card this sits on, which reads as no
      // box at all. --muted is ~4.8:1, and a dashed line inks about half its
      // pixels, so it lands as a quiet border rather than a loud one.
      <span
        className={`grid place-items-center rounded-(--r-xs) border border-dashed border-(--muted) text-(--fg-2) ${className ?? ''}`}
      >
        <svg viewBox="0 0 24 24" fill="none" className="size-3/5" aria-hidden>
          <path d="M12 5v14M5 12h14" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" />
        </svg>
      </span>
    )
  }

  if (isRaster(vendor)) {
    return (
      <span className={`grid place-items-center ${className ?? ''}`} aria-hidden>
        <img src={RASTER[vendor]} alt="" className="size-full object-contain" />
      </span>
    )
  }

  const { Mark, color } = VENDORS[vendor]
  return (
    <span
      className={`grid place-items-center ${className ?? ''}`}
      style={color ? { color } : undefined}
      aria-hidden
    >
      <Mark className="size-full" />
    </span>
  )
}
