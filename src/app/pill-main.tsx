import '../index.css'
import { PillApp } from '../pill/PillApp'
import { PillRecovery } from '../pill/PillRecovery'
import { bootstrapMirror } from './mirrorBootstrap'

bootstrapMirror(() => <PillApp />, { fallback: (message, retry) => <PillRecovery message={message} retry={retry} /> })
