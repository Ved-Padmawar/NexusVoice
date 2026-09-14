import { useState, useEffect, useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { motion, AnimatePresence } from 'framer-motion'
import { Database, HardDrive, Trash2 } from 'lucide-react'
import { toast } from 'sonner'
import { COMMANDS } from '../lib/commands'
import { fetchDownloadedModels, formatModelSize } from '../lib/models'
import type { DownloadedModel } from '../types'
import { IconButton } from './ui/button'
import { Modal, ModalBody, ModalFoot } from './ui/modal'

export function ModelManagerModal({ onClose }: { onClose: () => void }) {
  const [models, setModels] = useState<DownloadedModel[]>([])
  const [deleting, setDeleting] = useState<string | null>(null)

  const load = useCallback(() => { void fetchDownloadedModels().then(setModels) }, [])

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
    <Modal
      onClose={onClose}
      title="Downloaded models"
      description={models.length > 0
        ? `${models.length} ${models.length === 1 ? 'model' : 'models'}, ${formatModelSize(totalBytes)} on disk`
        : 'No models downloaded'}
      icon={<span className="nv-mark nv-mark--accent"><Database size={16} strokeWidth={2} /></span>}
    >
      <ModalBody className="flex flex-col gap-2">
        <AnimatePresence initial={false}>
          {models.length === 0 ? (
            <p className="py-6 text-center text-[13px] text-muted">Nothing on disk yet.</p>
          ) : (
            models.map((model) => (
              <motion.div
                key={model.variant}
                layout
                initial={{ opacity: 0, y: -6 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, x: -10 }}
                transition={{ duration: 0.18, ease: 'easeOut' }}
                className="flex items-center gap-3 rounded-lg bg-surface px-3.5 py-3 shadow-[inset_0_0_0_1px_var(--border-soft)]"
              >
                <HardDrive size={15} strokeWidth={1.8} className="shrink-0 text-faint" />
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="truncate text-[13px] font-semibold text-fg">{model.displayName}</span>
                    {model.isActive && <span className="nv-badge">In use</span>}
                  </div>
                  <span className="text-[12px] text-muted tabular-nums">{formatModelSize(model.sizeBytes)}</span>
                </div>
                <IconButton
                  label={`Delete ${model.displayName}`}
                  tone="danger"
                  disabled={deleting === model.variant}
                  onClick={() => handleDelete(model.variant, model.displayName)}
                >
                  <Trash2 strokeWidth={1.9} />
                </IconButton>
              </motion.div>
            ))
          )}
        </AnimatePresence>
      </ModalBody>
      <ModalFoot>
        <p className="nv-hint">
          Deleting the model in use frees its memory; transcription will ask you to pick another.
        </p>
      </ModalFoot>
    </Modal>
  )
}
