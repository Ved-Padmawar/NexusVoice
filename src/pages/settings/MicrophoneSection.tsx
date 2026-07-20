import { useState, useEffect, useCallback, useRef, memo } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { motion, AnimatePresence } from 'framer-motion'
import { Mic, RefreshCw, Check, ChevronDown } from 'lucide-react'
import { toast } from 'sonner'
import { COMMANDS } from '../../lib/commands'
import { extractErrorMessage } from '../../lib/errors'

type InputDevice = {
  name: string
  isDefault: boolean
  isSelected: boolean
}

/** Sentinel for the "Default" option — maps to no saved preference. */
const DEFAULT_VALUE = '__default__'

export const MicrophoneSection = memo(function MicrophoneSection() {
  const [devices, setDevices] = useState<InputDevice[]>([])
  const [selected, setSelected] = useState<string>(DEFAULT_VALUE)
  const [initialLoading, setInitialLoading] = useState(true)
  const [refreshing, setRefreshing] = useState(false)
  const [open, setOpen] = useState(false)
  const rootRef = useRef<HTMLDivElement>(null)

  const fetchDevices = useCallback(async () => {
    const list = await invoke<InputDevice[]>(COMMANDS.LIST_INPUT_DEVICES)
    const devices = Array.isArray(list) ? list : []
    setDevices(devices)
    const active = devices.find(d => d.isSelected && !d.isDefault)
    setSelected(active ? active.name : DEFAULT_VALUE)
  }, [])

  useEffect(() => {
    fetchDevices()
      .catch(e => toast.error(extractErrorMessage(e, 'Could not list microphones')))
      .finally(() => setInitialLoading(false))
  }, [fetchDevices])

  // Refresh keeps the controls interactive; only the icon spins. A minimum
  // duration guarantees the spin is visible even when the device list is cached.
  const refresh = useCallback(() => {
    if (refreshing) return
    setRefreshing(true)
    Promise.all([
      fetchDevices().catch(e => toast.error(extractErrorMessage(e, 'Could not list microphones'))),
      new Promise(r => setTimeout(r, 500)),
    ]).finally(() => setRefreshing(false))
  }, [refreshing, fetchDevices])

  // Close on outside click or Escape.
  useEffect(() => {
    if (!open) return
    const onDown = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) setOpen(false)
    }
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') setOpen(false) }
    document.addEventListener('mousedown', onDown)
    document.addEventListener('keydown', onKey)
    return () => {
      document.removeEventListener('mousedown', onDown)
      document.removeEventListener('keydown', onKey)
    }
  }, [open])

  const choose = useCallback(async (value: string) => {
    setOpen(false)
    if (value === selected) return
    const previous = selected
    setSelected(value)
    try {
      await invoke<void>(COMMANDS.SET_INPUT_DEVICE, {
        name: value === DEFAULT_VALUE ? null : value,
      })
    } catch (e) {
      setSelected(previous)
      toast.error(extractErrorMessage(e, 'Could not set microphone'))
    }
  }, [selected])

  const defaultLabel = devices.find(d => d.isDefault)?.name
  const currentLabel = selected === DEFAULT_VALUE
    ? (defaultLabel ? `Default — ${defaultLabel}` : 'Default')
    : selected

  // The default device is represented by the "Default — <name>" sentinel, so
  // list only the non-default devices by name (deduped) to avoid a duplicate row.
  const seen = new Set<string>()
  const options = [
    { value: DEFAULT_VALUE, label: defaultLabel ? `Default — ${defaultLabel}` : 'Default' },
    ...devices
      .filter(d => !d.isDefault && !d.name.startsWith('Default') && (seen.has(d.name) ? false : seen.add(d.name)))
      .map(d => ({ value: d.name, label: d.name })),
  ]

  return (
    <div className="flex flex-col gap-4">
      <div>
        <p className="text-[12px] font-semibold text-[var(--fg-2)] tracking-[-0.01em] mb-1">Microphone</p>
        <p className="text-[12px] text-[var(--muted)]">Choose which input device records your voice.</p>
      </div>

      <div className="flex items-center gap-2">
        <div ref={rootRef} className="relative flex-1">
          <button
            type="button"
            disabled={initialLoading}
            onClick={() => setOpen(o => !o)}
            className={`flex items-center w-full h-9 pl-8 pr-8 rounded-(--r-md) bg-(--surface) border text-[12px] text-(--fg) cursor-pointer text-left transition-[border-color] duration-(--t-fast) focus:outline-none disabled:opacity-50 ${open ? 'border-(--accent)' : 'border-(--border-soft) hover:border-(--border)'}`}
            aria-haspopup="listbox"
            aria-expanded={open}
          >
            <Mic size={14} strokeWidth={2} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-(--accent) pointer-events-none" />
            <span className="truncate">{currentLabel}</span>
            <motion.span
              className="absolute right-2.5 top-1/2 -translate-y-1/2 text-(--fg-2) pointer-events-none flex"
              animate={{ rotate: open ? 180 : 0 }}
              transition={{ duration: 0.18 }}
            >
              <ChevronDown size={14} strokeWidth={2} />
            </motion.span>
          </button>

          <AnimatePresence>
            {open && (
              <motion.ul
                role="listbox"
                initial={{ opacity: 0, y: -4, scale: 0.98 }}
                animate={{ opacity: 1, y: 0, scale: 1 }}
                exit={{ opacity: 0, y: -4, scale: 0.98 }}
                transition={{ duration: 0.14, ease: 'easeOut' }}
                className="absolute z-50 mt-1 w-full max-h-56 overflow-y-auto rounded-(--r-md) bg-(--panel) border border-(--border-soft) shadow-[var(--shadow-lg)] p-1"
              >
                {options.map(opt => {
                  const active = opt.value === selected
                  return (
                    <li key={opt.value} role="option" aria-selected={active}>
                      <button
                        type="button"
                        onClick={() => void choose(opt.value)}
                        className="flex items-center gap-2 w-full px-2 py-1.5 rounded-(--r-sm) text-[12px] text-left cursor-pointer transition-colors duration-(--t-fast) text-(--fg) hover:bg-(--surface-hover)"
                      >
                        <span className="flex-1 truncate">{opt.label}</span>
                        {active && <Check size={12} strokeWidth={3} className="text-(--accent) shrink-0" />}
                      </button>
                    </li>
                  )
                })}
              </motion.ul>
            )}
          </AnimatePresence>
        </div>

        <motion.button
          type="button"
          onClick={refresh}
          disabled={initialLoading}
          title="Refresh device list"
          whileTap={{ scale: 0.92 }}
          className="inline-flex items-center justify-center size-9 rounded-(--r-md) bg-(--surface) border border-(--border-soft) text-(--fg-2) cursor-pointer transition-[color,border-color] duration-(--t-fast) hover:text-(--fg) hover:border-(--border) disabled:opacity-50"
        >
          <motion.span
            className="flex"
            animate={refreshing ? { rotate: 360 } : { rotate: 0 }}
            transition={refreshing
              ? { repeat: Infinity, ease: 'linear', duration: 0.7 }
              : { duration: 0.2 }}
          >
            <RefreshCw size={13} strokeWidth={1.75} />
          </motion.span>
        </motion.button>
      </div>
    </div>
  )
})
