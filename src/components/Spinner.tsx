/** CSS-only ring spinner. No framer-motion in the window UI: a mount-time
 *  JS animation is exactly what keeps `document.getAnimations()` busy after a
 *  navigation, which is the metric that tracks perceived speed here. */
export function Spinner({ size = 16, className }: { size?: number; className?: string }) {
  return (
    <span
      role="presentation"
      className={`inline-block shrink-0 animate-spin rounded-full border-2 border-(--border) border-t-(--accent) ${className ?? ''}`}
      style={{ width: size, height: size }}
    />
  )
}
