import { useLayoutEffect, useRef } from 'react'
import { commands } from '../shared/bindings'

export function PillRecovery({ message, retry }: { message: string; retry: () => void }) {
  const button = useRef<HTMLButtonElement>(null)
  useLayoutEffect(() => {
    const { x, y, width, height } = button.current!.getBoundingClientRect()
    void commands.setPillHitbox({ x, y, width, height }).catch(() => {})
    return () => { void commands.setPillHitbox({ x: 0, y: 0, width: 0, height: 0 }).catch(() => {}) }
  }, [])
  return (
    <div role="alert">
      <button ref={button} type="button" title={message} aria-label={`${message} Try again`} onClick={retry}
        className="h-8 w-26 cursor-pointer rounded-full bg-[#0f0f18] text-xs font-semibold text-white ring-1 ring-white/15 ring-inset">
        Retry sync
      </button>
    </div>
  )
}
