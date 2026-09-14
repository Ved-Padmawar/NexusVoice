import { useState } from 'react'
import { Popover } from 'radix-ui'
import { ArrowUpCircle, Download, RotateCcw } from 'lucide-react'
import { useAppStore } from '../../store/useAppStore'
import { Button } from '../ui/button'
import { PopoverPanel } from '../ui/popover'

/** Installs in place. Dismissing hides it for the session; About still shows it. */
export function UpdateChip() {
  const status = useAppStore(s => s.updateStatus)
  const version = useAppStore(s => s.updateVersion)
  const progress = useAppStore(s => s.updateProgress)
  const dismissed = useAppStore(s => s.updateDismissed)
  const installUpdate = useAppStore(s => s.installUpdate)
  const restartForUpdate = useAppStore(s => s.restartForUpdate)
  const dismissUpdate = useAppStore(s => s.dismissUpdate)
  const [open, setOpen] = useState(false)

  const shown = !dismissed && (status === 'available' || status === 'downloading' || status === 'ready')
  if (!shown) return null

  const chipLabel = status === 'ready' ? 'Restart to update' : status === 'downloading' ? `Updating ${progress}%` : 'Update'

  return (
    <Popover.Root open={open} onOpenChange={setOpen}>
      <Popover.Trigger asChild>
        <button type="button" className="nv-update-chip">
          {status === 'available' ? <span className="nv-dot" /> : <ArrowUpCircle strokeWidth={2} />}
          <span className="tabular-nums">{chipLabel}</span>
        </button>
      </Popover.Trigger>
      <PopoverPanel open={open} className="w-72">
        <div className="nv-pop-body">
          <div>
            <p className="text-[14px] font-semibold text-fg">
              {status === 'ready' ? 'Update installed' : status === 'downloading' ? 'Downloading update' : 'A new version is ready'}
            </p>
            <p className="mt-1 text-[12.5px] text-muted tabular-nums">
              {status === 'ready'
                ? `Restart NexusVoice to finish moving to v${version}.`
                : `v${__APP_VERSION__} to v${version}`}
            </p>
          </div>

          {status === 'downloading' ? (
            <div className="flex items-center gap-3">
              <span className="nv-progress">
                <span className="nv-progress__fill block transition-[width] duration-300" style={{ width: `${progress}%` }} />
              </span>
              <span className="text-[12px] font-semibold tabular-nums text-accent-text">{progress}%</span>
            </div>
          ) : (
            <div className="flex items-center gap-2">
              <Button
                size="sm"
                className="flex-1"
                onClick={status === 'ready' ? restartForUpdate : installUpdate}
              >
                {status === 'ready' ? <><RotateCcw />Restart now</> : <><Download />Install update</>}
              </Button>
              <Button size="sm" variant="ghost" onClick={() => { setOpen(false); dismissUpdate() }}>
                Not now
              </Button>
            </div>
          )}
        </div>
      </PopoverPanel>
    </Popover.Root>
  )
}
