import { memo } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { Check } from 'lucide-react'
import { useAppStore, type ThemeName } from '../../store/useAppStore'

const THEMES: {
  name: ThemeName; label: string; mode: 'dark' | 'light'
  bg: string; panel: string; accent: string; border: string; surface: string; muted: string
}[] = [
  { name: 'abyss',    label: 'Abyss',    mode: 'dark',  bg: '#22232b', panel: '#2a2b35', accent: '#78a2f4', border: '#383a48', surface: '#2e303d', muted: '#6a6e88' },
  { name: 'midnight', label: 'Midnight', mode: 'dark',  bg: '#0d101c', panel: '#131526', accent: '#8b5cf6', border: '#1e2240', surface: '#181b2e', muted: '#4a507a' },
  { name: 'nebula',   label: 'Nebula',   mode: 'dark',  bg: '#231b2a', panel: '#2c2433', accent: '#9d38a8', border: '#3e3048', surface: '#332840', muted: '#6a5878' },
  { name: 'pine',     label: 'Pine',     mode: 'dark',  bg: '#1b2420', panel: '#222d29', accent: '#58c596', border: '#304038', surface: '#283530', muted: '#507060' },
  { name: 'canvas',   label: 'Canvas',   mode: 'light', bg: '#f8f9fc', panel: '#ffffff', accent: '#3a5bd9', border: '#d8dce8', surface: '#f0f2f8', muted: '#8890b0' },
  { name: 'dawn',     label: 'Dawn',     mode: 'light', bg: '#faf4ee', panel: '#ede0d0', accent: '#d4610a', border: '#d8c8b4', surface: '#f5ede2', muted: '#9a8870' },
  { name: 'breeze',   label: 'Breeze',   mode: 'light', bg: '#eef6f8', panel: '#d8eef0', accent: '#1a7a8a', border: '#c0d8dc', surface: '#e8f4f6', muted: '#6a9098' },
  { name: 'blossom',  label: 'Blossom',  mode: 'light', bg: '#f8eef0', panel: '#e8d4d8', accent: '#c0304a', border: '#d8c0c4', surface: '#f2e4e8', muted: '#9a7078' },
]

function ThemePreview({ bg, panel, accent, border, surface, muted }: {
  bg: string; panel: string; accent: string; border: string; surface: string; muted: string
}) {
  return (
    <div className="relative h-[78px] w-full overflow-hidden rounded-t-[var(--r-md)]" style={{ background: bg }}>
      {/* Titlebar */}
      <div className="flex h-[9px] items-center gap-[2px] px-1" style={{ background: panel }}>
        <div className="size-[3px] rounded-full opacity-50" style={{ background: muted }} />
        <div className="size-[3px] rounded-full opacity-50" style={{ background: muted }} />
        <div className="size-[3px] rounded-full opacity-50" style={{ background: muted }} />
        <div className="ml-auto h-[3px] w-[28px] rounded-full opacity-60" style={{ background: border }} />
      </div>
      {/* Sidebar */}
      <div className="absolute bottom-0 left-0 top-[9px] w-[22px]" style={{ background: panel, borderRight: `0.5px solid ${border}` }}>
        <div className="mx-[3px] mt-[5px] h-[2px] w-[10px] rounded-[1px] opacity-70" style={{ background: accent }} />
        <div className="mx-[3px] mt-1 h-[2px] w-[8px] rounded-[1px] opacity-30" style={{ background: muted }} />
        <div className="mx-[3px] mt-1 h-[2px] w-[9px] rounded-[1px] opacity-30" style={{ background: muted }} />
      </div>
      {/* Content */}
      <div className="absolute bottom-1 left-[26px] right-1 top-[13px]">
        <div className="mb-1 h-[2.5px] w-[55%] rounded-[1px] opacity-40" style={{ background: muted }} />
        <div className="mb-1 flex gap-[3px]">
          <div className="flex h-[11px] flex-1 items-end justify-center rounded-[2px] pb-[1.5px]" style={{ background: surface, border: `0.5px solid ${border}` }}>
            <div className="h-[2px] w-[60%] rounded-[1px] opacity-70" style={{ background: accent }} />
          </div>
          <div className="flex h-[11px] flex-1 items-end justify-center rounded-[2px] pb-[1.5px]" style={{ background: surface, border: `0.5px solid ${border}` }}>
            <div className="h-[2px] w-[55%] rounded-[1px] opacity-40" style={{ background: accent }} />
          </div>
          <div className="flex h-[11px] flex-1 items-end justify-center rounded-[2px] pb-[1.5px]" style={{ background: surface, border: `0.5px solid ${border}` }}>
            <div className="h-[2px] w-[50%] rounded-[1px] opacity-25" style={{ background: accent }} />
          </div>
        </div>
        <div className="rounded-[2px] p-[3px] px-1" style={{ background: surface, border: `0.5px solid ${border}`, height: 'calc(100% - 18px)' }}>
          <div className="mb-[3px] h-[1.5px] w-[50%] rounded-[1px] opacity-35" style={{ background: muted }} />
          <div className="mb-[3px] h-[1.5px] w-[70%] rounded-[1px] opacity-20" style={{ background: muted }} />
          <div className="h-[1.5px] w-[35%] rounded-[1px] opacity-30" style={{ background: accent }} />
        </div>
      </div>
    </div>
  )
}

export const GeneralTab = memo(function GeneralTab() {
  const { theme, setTheme } = useAppStore()

  return (
    <div className="flex flex-col gap-5">
      <div>
        <p className="text-[11px] font-semibold text-[var(--fg-2)] uppercase tracking-[0.03em] mb-3">Appearance</p>
        <p className="text-[12px] text-[var(--muted)] mb-4">Choose a color scheme for your workspace.</p>

        {(['dark', 'light'] as const).map((mode) => {
          const group = THEMES.filter(t => t.mode === mode)
          return (
            <div key={mode} className="mb-5 last:mb-0">
              <p className="text-[10px] font-semibold text-[var(--muted)] uppercase tracking-[0.06em] mb-2">{mode === 'dark' ? 'Dark' : 'Light'}</p>
              <div className="grid gap-2" style={{ gridTemplateColumns: 'repeat(4, minmax(120px, 160px))' }}>
                {group.map((t) => {
                  const active = theme === t.name
                  return (
                    <motion.button
                      key={t.name}
                      type="button"
                      className="flex flex-col p-0 rounded-[var(--r-md)] border-[1.5px] cursor-pointer text-left overflow-hidden"
                      onClick={() => setTheme(t.name)}
                      initial={false}
                      animate={{
                        borderColor: active ? 'var(--accent)' : 'var(--border-soft)',
                        boxShadow: active ? '0 0 0 1px var(--accent)' : '0 0 0 0px transparent',
                        backgroundColor: 'var(--surface)',
                      }}
                      whileHover={{ y: -1, boxShadow: active ? '0 0 0 1px var(--accent)' : 'var(--shadow-md)' }}
                      whileTap={{ scale: 0.99 }}
                      transition={{ type: 'spring', stiffness: 300, damping: 25, mass: 0.8 }}
                    >
                      <ThemePreview bg={t.bg} panel={t.panel} accent={t.accent} border={t.border} surface={t.surface} muted={t.muted} />
                      <div className="flex items-center justify-between px-[6px] py-[4px]">
                        <motion.span
                          className="text-[10px] font-semibold tracking-[-0.01em]"
                          initial={false}
                          animate={{ color: active ? 'var(--accent)' : 'var(--fg)' }}
                          transition={{ duration: 0.2 }}
                        >
                          {t.label}
                        </motion.span>
                        <AnimatePresence>
                          {active && (
                            <motion.span
                              initial={{ scale: 0, opacity: 0 }}
                              animate={{ scale: 1, opacity: 1 }}
                              exit={{ scale: 0, opacity: 0 }}
                              transition={{ type: 'spring', stiffness: 300, damping: 25, mass: 0.8 }}
                            >
                              <Check size={8} strokeWidth={3.5} className="text-[var(--accent)] flex-shrink-0" />
                            </motion.span>
                          )}
                        </AnimatePresence>
                      </div>
                    </motion.button>
                  )
                })}
              </div>
            </div>
          )
        })}

      </div>
    </div>
  )
})
