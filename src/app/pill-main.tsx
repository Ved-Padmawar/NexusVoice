import '../index.css'
import { PillApp } from '../pill/PillApp'
import { PillRecovery } from '../pill/PillRecovery'
import { bootstrapMirror } from './mirrorBootstrap'

// The pill shows the recording, its own settings and download progress.
bootstrapMirror(['settings', 'recording', 'downloads'], () => <PillApp />, { fallback: (message, retry) => <PillRecovery message={message} retry={retry} /> })
