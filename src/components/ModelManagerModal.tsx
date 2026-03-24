import { useState, useEffect, useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { motion, AnimatePresence } from 'framer-motion'
import { HardDrive, Trash2, X, Database } from 'lucide-react'
import { COMMANDS } from '../lib/commands'
import { toast } from 'sonner'

type DownloadedModel = {
  variant: string
  displayName: string
  sizeBytes: number
  isActive: boolean
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B'
  const gb = bytes / (1024 ** 3)
  if (gb >= 1) return `${gb.toFixed(2)} GB`
  const mb = bytes / (1024 ** 2)
  return `${mb.toFixed(0)} MB`
}

type Props = {
  onClose: () => void
}

export function ModelManagerModal({ onClose }: Props) {
  const [models, setModels] = useState<DownloadedModel[]>([])
  const [deleting, setDeleting] = useState<string | null>(null)

  const load = useCallback(() => {
    invoke<DownloadedModel[]>(COMMANDS.GET_DOWNLOADED_MODELS)
      .then(setModels)
      .catch(() => {})
  }, [])

  useEffect(() => { load() }, [load])

  // Close on Escape
  useEffect(() => {
    const handler = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose() }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [onClose])

  const handleDelete = async (variant: string, displayName: string) => {
    setDeleting(variant)
    try {
      await invoke(COMMANDS.DELETE_MODEL, { variant })
      toast.success(`${displayName} deleted`)
      load()
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e)
      toast.error(msg.includes('active_model') ? 'Cannot delete the active model' : 'Failed to delete model')
    } finally {
      setDeleting(null)
    }
  }

  const totalBytes = models.reduce((acc, m) => acc + m.sizeBytes, 0)

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-[2px]"
      onClick={(e) => { if (e.target === e.currentTarget) onClose() }}
    >
      <motion.div
        initial={{ opacity: 0, scale: 0.96, y: 8 }}
        animate={{ opacity: 1, scale: 1, y: 0 }}
        exit={{ opacity: 0, scale: 0.96, y: 8 }}
        transition={{ duration: 0.22, ease: 'easeOut' }}
        className="w-[460px] flex flex-col bg-[var(--panel)] border border-[var(--border)] rounded-[var(--r-xl)] shadow-[var(--shadow-lg)] overflow-hidden"
      >
        {/* Header */}
        <div className="flex items-center justify-between px-6 pt-6 pb-4 border-b border-[var(--border-soft)]">
          <div className="flex items-center gap-3">
            <div className="w-9 h-9 rounded-[var(--r-lg)] bg-[var(--accent-soft)] text-[var(--accent)] flex items-center justify-center flex-shrink-0">
              <Database size={15} strokeWidth={2} />
            </div>
            <div>
              <h2 className="text-[15px] font-bold tracking-[-0.025em] text-[var(--fg)] m-0">Model Manager</h2>
              <p className="text-[11px] text-[var(--muted)] mt-[2px]">
                {models.length > 0 ? `${models.length} model${models.length > 1 ? 's' : ''} · ${formatBytes(totalBytes)} on disk` : 'No models downloaded'}
              </p>
            </div>
          </div>
          <button
            type="button"
            aria-label="Close"
            className="flex items-center justify-center w-7 h-7 rounded-[var(--r-md)] text-[var(--muted)] bg-transparent border-none cursor-pointer transition-[color,background] duration-[var(--t-fast)] hover:text-[var(--fg)] hover:bg-[var(--surface-hover)]"
            onClick={onClose}
          >
            <X size={14} strokeWidth={2} />
          </button>
        </div>

        {/* Model list */}
        <div className="flex flex-col gap-2 px-6 py-5">
          <AnimatePresence initial={false}>
            {models.length === 0 ? (
              <p className="text-[12px] text-[var(--muted)] text-center py-6">No models on disk.</p>
            ) : (
              models.map((model) => (
                <motion.div
                  key={model.variant}
                  layout
                  initial={{ opacity: 0, y: -6 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, x: -8 }}
                  transition={{ duration: 0.18, ease: 'easeOut' }}
                  className="flex items-center gap-3 px-4 py-3 rounded-[var(--r-lg)] bg-[var(--surface)] border border-[var(--border-soft)]"
                >
                  <HardDrive size={13} strokeWidth={1.75} className="text-[var(--muted)] flex-shrink-0" />

                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="text-[12px] font-semibold text-[var(--fg)]">{model.displayName}</span>
                      {model.isActive && (
                        <span className="text-[9px] font-bold text-[var(--accent)] bg-[var(--accent-soft)] border border-[var(--accent-soft)] rounded-[var(--r-xs)] px-[5px] py-px uppercase tracking-[0.04em]">
                          Active
                        </span>
                      )}
                    </div>
                    <span className="text-[10px] text-[var(--muted)]">{formatBytes(model.sizeBytes)}</span>
                  </div>

                  <button
                    type="button"
                    aria-label={`Delete ${model.displayName}`}
                    disabled={model.isActive || deleting === model.variant}
                    className="flex items-center justify-center w-7 h-7 rounded-[var(--r-md)] text-[var(--muted)] bg-transparent border-none cursor-pointer transition-[color,background] duration-[var(--t-fast)] hover:text-[var(--danger)] hover:bg-[var(--surface-hover)] disabled:opacity-30 disabled:cursor-not-allowed"
                    onClick={() => handleDelete(model.variant, model.displayName)}
                  >
                    <Trash2 size={13} strokeWidth={1.75} />
                  </button>
                </motion.div>
              ))
            )}
          </AnimatePresence>
        </div>

        {/* Footer */}
        <div className="px-6 pb-5 pt-0">
          <p className="text-[10px] text-[var(--muted)]">The active model cannot be deleted. Switch models in the Whisper Model section above.</p>
        </div>
      </motion.div>
    </div>
  )
}
