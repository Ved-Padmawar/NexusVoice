/** Chrome lives in `.switch` (index.css) so the knob and track follow the
 *  active theme rather than a hardcoded white. */
export function Switch({
  checked,
  onChange,
  label,
}: {
  checked: boolean
  onChange: () => void
  label: string
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      onClick={onChange}
      className="switch"
    />
  )
}
