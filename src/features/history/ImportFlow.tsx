import { useEffect, useRef, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { open } from '@tauri-apps/plugin-dialog'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { FileAudioIcon, UploadSimpleIcon, WaveformIcon, XIcon } from '@phosphor-icons/react'
import { toast } from 'sonner'
import { commands, type AudioFileInfo } from '../../shared/bindings'
import { extractErrorMessage } from '../../shared/errors'
import { queryKeys } from '../../shared/queryKeys'
import { fmtClock } from '../../shared/utils'
import { Button } from '../../shared/ui/Button'
import { Modal } from '../../shared/ui/Modal'
import { Select } from '../../shared/ui/Select'
import { useDownloadedModels } from './jobs'

const EXTENSIONS = ['wav', 'mp3', 'm4a', 'aac', 'flac', 'ogg', 'opus']

type PickedFile = AudioFileInfo & { path: string }

const fmtSize = (bytes: number) =>
  bytes >= 1_048_576 ? `${(bytes / 1_048_576).toFixed(1)} MB` : `${Math.max(1, Math.round(bytes / 1024))} KB`

/** Read what the dialog shows about a file, or say why it can't be imported. */
async function inspect(path: string): Promise<PickedFile | null> {
  try {
    return { ...(await commands.inspectAudioFile(path)), path }
  } catch (e) {
    toast.error(extractErrorMessage(e, 'Could not read that file'))
    return null
  }
}

/** The file to import: picked from the dialog or dropped on the page. Only
 *  the latest pick can open the dialog; a newer pick, closing the dialog or
 *  starting to read a file voids any inspection still under way. */
function useImportFile() {
  const [file, setFile] = useState<PickedFile | null>(null)
  const [dragging, setDragging] = useState(false)
  const [reading, setReadingState] = useState(false)
  // Refs as well, for the drop listener and inspections that outlive a render.
  const readingRef = useRef(false)
  const latestPick = useRef(0)

  const setReading = (next: boolean) => {
    readingRef.current = next
    if (next) latestPick.current += 1
    setReadingState(next)
  }

  const offer = async (path: string) => {
    if (readingRef.current) return
    const ticket = ++latestPick.current
    const picked = await inspect(path)
    if (picked && ticket === latestPick.current) setFile(picked)
  }

  const pick = async () => {
    const path = await open({ multiple: false, filters: [{ name: 'Audio', extensions: EXTENSIONS }] })
    if (path) await offer(path)
  }

  useEffect(() => {
    const unlisten = getCurrentWebview().onDragDropEvent(async ({ payload }) => {
      if (payload.type === 'enter') setDragging(!readingRef.current)
      else if (payload.type === 'leave') setDragging(false)
      else if (payload.type === 'drop') {
        setDragging(false)
        const [path] = payload.paths
        if (path) await offer(path)
      }
    })
    return () => { void unlisten.then(fn => fn()) }
  }, [])

  const close = () => {
    latestPick.current += 1
    setFile(null)
  }

  return { file, close, pick, dragging, reading, setReading }
}

/** The toolbar's import button, with the dialog and the drop target it opens. */
export function ImportControls() {
  const { file, close, pick, dragging, reading, setReading } = useImportFile()
  return (
    <>
      <ImportButton disabled={reading} onClick={() => void pick()} />
      <ImportModal key={file?.path} file={file} reading={reading} onReading={setReading} onClose={close} />
      <DropOverlay show={dragging} />
    </>
  )
}

function ImportButton({ disabled, onClick }: { disabled: boolean; onClick: () => void }) {
  return (
    <span data-tip={disabled ? 'Reading the file…' : 'Transcribe an audio file into History'} data-tip-align="end">
      <Button variant="chip" size="icon" aria-label="Import audio" disabled={disabled} onClick={onClick}>
        <UploadSimpleIcon size={15} />
      </Button>
    </span>
  )
}

function DropOverlay({ show }: { show: boolean }) {
  return (
    <div
      aria-hidden="true"
      className={`pointer-events-none fixed inset-3 z-40 grid place-items-center rounded-2xl bg-[color-mix(in_oklab,var(--bg)_82%,transparent)] ring-2 ring-accent/60 transition-opacity duration-160 ring-inset ${show ? 'opacity-100' : 'opacity-0'}`}
    >
      <div className="flex flex-col items-center gap-2 text-accent-ink">
        <FileAudioIcon size={36} weight="duotone" />
        <p className="text-md font-semibold">Drop to transcribe</p>
      </div>
    </div>
  )
}

const label = 'text-xs font-semibold text-fg-2'

type ImportModalProps = {
  file: PickedFile | null
  /** Rust is reading the file; it can't be stopped, so the dialog stays until done. */
  reading: boolean
  onReading: (reading: boolean) => void
  onClose: () => void
}

function ImportModal({ file, reading, onReading, onClose }: ImportModalProps) {
  const models = useDownloadedModels(file !== null)
  const [picked, setPicked] = useState<string | null>(null)
  const [language, setLanguage] = useState<string | null>(null)

  const options = models.data ?? []
  const model = picked ?? options.find(m => m.isActive)?.variant ?? options[0]?.variant ?? null
  const languages = useQuery({
    queryKey: [...queryKeys.languageOptions, model],
    queryFn: () => commands.getLanguageOptionsFor(model ?? ''),
    enabled: file !== null && model !== null,
  })
  const codes = languages.data?.supported ? languages.data.options : []
  const chosen = language ?? codes.find(o => o.isSelected)?.code ?? null

  const start = async () => {
    if (!file || !model) return
    onReading(true)
    try {
      await commands.importAudioFile(file.path, model, chosen)
      onClose()
    } catch (e) {
      toast.error(extractErrorMessage(e, 'Could not start the transcription'))
    } finally {
      onReading(false)
    }
  }

  const close = () => { if (!reading) onClose() }

  return (
    <Modal open={file !== null} onClose={close} aria-labelledby="import-title" className="w-[420px]">
      <div className="flex items-center justify-between px-5 pt-4.5">
        <h2 id="import-title" className="text-md font-semibold tracking-tight text-fg">Transcribe a file</h2>
        <button type="button" aria-label="Close" disabled={reading} onClick={close} className="grid size-7 cursor-pointer place-items-center rounded-lg text-muted hover:bg-surface-hover hover:text-fg disabled:pointer-events-none disabled:opacity-40">
          <XIcon size={13} weight="bold" />
        </button>
      </div>
      <div className="flex flex-col gap-4 px-5 pt-4 pb-5">
        <div className="flex items-center gap-3 rounded-xl bg-surface p-3 ring-1 ring-line-soft ring-inset">
          <span className="grid size-9 shrink-0 place-items-center rounded-lg bg-accent-soft text-accent-ink"><FileAudioIcon size={18} weight="duotone" /></span>
          <div className="min-w-0 flex-1">
            <p className="truncate text-sm font-semibold text-fg">{file?.name}</p>
            <p className="text-xs text-muted tabular-nums">
              {file?.durationSeconds != null && `${fmtClock(file.durationSeconds)} · `}{file && fmtSize(file.sizeBytes)}
            </p>
          </div>
        </div>
        <div className="flex flex-col gap-1.5">
          <span className={label}>Model</span>
          <Select
            value={model}
            aria-label="Model"
            options={options.map(m => ({ value: m.variant, label: m.isActive ? `${m.displayName} (active)` : m.displayName }))}
            onChange={(value) => { setPicked(value); setLanguage(null) }}
            disabled={options.length === 0}
          />
        </div>
        {codes.length > 0 && (
          <div className="flex flex-col gap-1.5">
            <span className={label}>Language</span>
            <Select value={chosen} aria-label="Language" options={codes.map(o => ({ value: o.code, label: o.name, dividerAfter: o.code === 'auto' }))} onChange={setLanguage} />
          </div>
        )}
        <p className="text-xs text-pretty text-muted">
          Runs in the background, so you can keep dictating; dictation always goes first. The transcript lands in History. Nothing is pasted.
        </p>
        <div className="flex justify-end gap-2">
          <Button variant="ghost" disabled={reading} onClick={close}>Cancel</Button>
          {/* Until the languages load, the saved one isn't known to preselect. */}
          <Button variant="primary" disabled={!model || languages.isPending || reading} onClick={() => void start()}>
            <WaveformIcon size={13} weight="bold" />{reading ? 'Reading file…' : 'Transcribe'}
          </Button>
        </div>
      </div>
    </Modal>
  )
}
