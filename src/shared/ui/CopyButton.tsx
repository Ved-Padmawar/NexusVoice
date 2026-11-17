import { CheckIcon, CopyIcon } from '@phosphor-icons/react'
import { toast } from 'sonner'
import { cn } from '@/shared/utils'

/** Copies `text`; the icon turns into a check for a moment. */
export function CopyButton({ text, label = 'Copy', className }: { text: string; label?: string; className?: string }) {
  const handleCopy = () => {
    navigator.clipboard.writeText(text).catch(() => toast.error('Could not copy to the clipboard'))
  }
  return (
    <button
      type="button"
      aria-label={label}
      onClick={handleCopy}
      className={cn('copy-btn grid size-7 shrink-0 cursor-pointer place-items-center rounded-lg text-muted hover:bg-surface-hover hover:text-fg', className)}
    >
      <CopyIcon size={15} className="c-idle" />
      <CheckIcon size={15} weight="bold" className="c-done" />
    </button>
  )
}
