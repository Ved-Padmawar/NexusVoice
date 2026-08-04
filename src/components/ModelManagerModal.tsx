import { useState, useEffect, useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { motion, AnimatePresence } from 'framer-motion'
import { Dialog } from 'radix-ui'
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

  const handleDelete = async (variant: string, displayName: string) => {
    setDeleting(variant)
    try {
      await invoke(COMMANDS.DELETE_MODEL, { variant })
      toast.success(`${displayName} deleted`)
      load()
    } catch {
      toast.error('Failed to delete model')
    } finally {
      setDeleting(null)
    }
  }

  const totalBytes = models.reduce((acc, m) => acc + m.sizeBytes, 0)

  return (
    <Dialog.Root open onOpenChange={(next) => { if (!next) onClose() }}>
      <Dialog.Portal forceMount>
        <Dialog.Overlay asChild>
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.18 }}
            className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-[2px]"
          />
        </Dialog.Overlay>
        <Dialog.Content
          aria-describedby={undefined}
          asChild
          onOpenAutoFocus={(e) => e.preventDefault()}
          className="fixed inset-0 z-50 flex items-center justify-center pointer-events-none"
        >
          <div>
            <motion.div
              initial={{ opacity: 0, scale: 0.96, y: 8 }}
              animate={{ opacity: 1, scale: 1, y: 0 }}
              exit={{ opacity: 0, scale: 0.96, y: 8 }}
              transition={{ duration: 0.22, ease: 'easeOut' }}
              className="w-115 flex flex-col bg-(--panel) border border-(--border) rounded-(--r-xl) shadow-(--shadow-lg) overflow-hidden pointer-events-auto"
            >
              {/* Header */}
              <div className="flex items-center justify-between px-6 pt-6 pb-4 border-b border-(--border-soft)">
                <div className="flex items-center gap-3">
                  <div className="w-9 h-9 rounded-(--r-lg) bg-(--accent-soft) text-(--accent) flex items-center justify-center shrink-0">
                    <Database size={15} strokeWidth={2} />
                  </div>
                  <div>
                    <Dialog.Title className="text-[15px] font-bold tracking-tight text-(--fg) m-0">Model Manager</Dialog.Title>
                    <p className="text-[11px] text-muted-foreground mt-0.5">
                      {models.length > 0 ? `${models.length} model${models.length > 1 ? 's' : ''} · ${formatBytes(totalBytes)} on disk` : 'No models downloaded'}
                    </p>
                  </div>
                </div>
                <Dialog.Close asChild>
                  <button
                    type="button"
                    aria-label="Close"
                    className="flex items-center justify-center w-7 h-7 rounded-(--r-md) text-muted-foreground bg-transparent border-none cursor-pointer transition-[color,background] duration-(--t-fast) hover:text-(--fg) hover:bg-accent"
                  >
                    <X size={14} strokeWidth={2} />
                  </button>
                </Dialog.Close>
              </div>

              {/* Model list */}
              <div className="flex flex-col gap-2 px-6 py-5">
                <AnimatePresence initial={false}>
                  {models.length === 0 ? (
                    <p className="text-[12px] text-muted-foreground text-center py-6">No models on disk.</p>
                  ) : (
                    models.map((model) => (
                      <motion.div
                        key={model.variant}
                        layout
                        initial={{ opacity: 0, y: -6 }}
                        animate={{ opacity: 1, y: 0 }}
                        exit={{ opacity: 0, x: -8 }}
                        transition={{ duration: 0.18, ease: 'easeOut' }}
                        className="flex items-center gap-3 px-4 py-3 rounded-(--r-lg) bg-(--surface) border border-(--border-soft)"
                      >
                        <HardDrive size={13} strokeWidth={1.75} className="text-muted-foreground shrink-0" />

                        <div className="flex-1 min-w-0">
                          <div className="flex items-center gap-2">
                            <span className="text-[12px] font-semibold text-(--fg)">{model.displayName}</span>
                            {model.isActive && (
                              <span className="text-[9px] font-bold text-(--accent) bg-(--accent-soft) border border-(--accent-soft) rounded-(--r-xs) px-1.25 py-px uppercase tracking-[0.04em]">
                                Active
                              </span>
                            )}
                          </div>
                          <span className="text-[10px] text-muted-foreground">{formatBytes(model.sizeBytes)}</span>
                        </div>

                        <button
                          type="button"
                          aria-label={`Delete ${model.displayName}`}
                          disabled={deleting === model.variant}
                          className="flex items-center justify-center w-7 h-7 rounded-(--r-md) text-muted-foreground bg-transparent border-none cursor-pointer transition-[color,background] duration-(--t-fast) hover:text-destructive hover:bg-accent disabled:opacity-30 disabled:cursor-not-allowed"
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
                <p className="text-[10px] text-muted-foreground">Deleting the active model frees its memory; transcription will prompt you to pick a model again.</p>
              </div>
            </motion.div>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  )
}
