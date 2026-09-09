import { useState, useEffect, useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Dialog } from 'radix-ui'
import { HardDrive, Trash2, X } from 'lucide-react'
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
      <Dialog.Portal>
        <Dialog.Overlay className="scrim fixed inset-0 z-50 data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=closed]:animate-out data-[state=closed]:fade-out-0" />
        <Dialog.Content
          aria-describedby={undefined}
          onOpenAutoFocus={(e) => e.preventDefault()}
          className="pop fixed left-1/2 top-1/2 z-50 flex w-115 -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden
                     data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95
                     data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95"
        >
          <div className="flex items-center justify-between gap-4 px-5 py-4">
            <div className="min-w-0">
              <Dialog.Title className="m-0 text-[14px] font-semibold tracking-[-0.01em] text-(--fg)">
                Downloaded models
              </Dialog.Title>
              <p className="m-0 mt-0.5 text-[11.5px] text-(--muted)">
                {models.length > 0
                  ? `${models.length} model${models.length > 1 ? 's' : ''}, ${formatBytes(totalBytes)} on disk`
                  : 'Nothing downloaded yet'}
              </p>
            </div>
            <Dialog.Close asChild>
              <button type="button" aria-label="Close" className="iconbtn iconbtn-danger">
                <X size={14} strokeWidth={2} />
              </button>
            </Dialog.Close>
          </div>
          <div className="rule h-px" />

          <div className="flex flex-col gap-1.5 p-4">
            {models.length === 0 ? (
              <p className="py-8 text-center text-[12px] text-(--muted)">
                Models you download appear here, so you can free the space later.
              </p>
            ) : (
              models.map((model) => (
                <div
                  key={model.variant}
                  className="group flex items-center gap-3 rounded-(--r-lg) bg-(--surface) px-3.5 py-2.5"
                >
                  <HardDrive size={13} strokeWidth={1.9} className="shrink-0 text-(--muted)" />

                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="truncate text-[12px] font-medium text-(--fg)">
                        {model.displayName}
                      </span>
                      {model.isActive && (
                        <span className="shrink-0 rounded-(--r-xs) bg-(--accent-soft) px-1.5 py-px text-[9.5px] font-semibold text-(--on-soft)">
                          In use
                        </span>
                      )}
                    </div>
                    <span className="text-[10.5px] tabular-nums text-(--muted)">
                      {formatBytes(model.sizeBytes)}
                    </span>
                  </div>

                  <button
                    type="button"
                    aria-label={`Delete ${model.displayName}`}
                    title={`Delete ${model.displayName}`}
                    disabled={deleting === model.variant}
                    onClick={() => handleDelete(model.variant, model.displayName)}
                    className="iconbtn iconbtn-danger row-actions"
                  >
                    <Trash2 size={13} strokeWidth={1.9} />
                  </button>
                </div>
              ))
            )}
          </div>

          <div className="rule h-px" />
          <p className="m-0 px-5 py-3.5 text-[10.5px] leading-[1.5] text-(--muted)">
            Deleting the model in use frees its memory. Transcription then asks you
            to pick another one.
          </p>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  )
}
