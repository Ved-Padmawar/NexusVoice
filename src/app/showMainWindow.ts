import { getCurrentWindow } from '@tauri-apps/api/window'

let shown = false
export function showMainWindow() {
  if (shown) return
  shown = true
  void getCurrentWindow().show().catch(() => { shown = false })
}
