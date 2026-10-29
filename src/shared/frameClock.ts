const FRAME_MS = 1000 / 30
// A step after a stall must not jump the animation forward.
const MAX_DT_SECONDS = 0.1

type FrameListener = (dt: number) => void

const listeners = new Set<FrameListener>()
let windowVisible = true
let timer: ReturnType<typeof setInterval> | undefined
let lastTick = 0

function tick() {
  const now = performance.now()
  const dt = Math.min((now - lastTick) / 1000, MAX_DT_SECONDS)
  lastTick = now
  for (const listener of listeners) listener(dt)
}

/** Runs the timer only while someone is listening and the window is shown. */
function sync() {
  const shouldRun = windowVisible && listeners.size > 0
  if (shouldRun && timer === undefined) {
    lastTick = performance.now()
    timer = setInterval(tick, FRAME_MS)
  } else if (!shouldRun && timer !== undefined) {
    clearInterval(timer)
    timer = undefined
  }
}

/** Hiding to the tray leaves `document.visibilityState` at `visible`. */
export function setWindowVisible(visible: boolean) {
  windowVisible = visible
  sync()
}

function subscribe(listener: FrameListener) {
  listeners.add(listener)
  sync()
  return () => {
    listeners.delete(listener)
    sync()
  }
}

/** Ticks `onFrame` on the shared ~30 fps clock while `element` is on screen. */
export function subscribeWhileVisible(element: Element, onFrame: FrameListener) {
  if (typeof IntersectionObserver === 'undefined') return subscribe(onFrame)

  let unsubscribe: (() => void) | undefined
  const observer = new IntersectionObserver(([entry]) => {
    if (entry.isIntersecting) unsubscribe ??= subscribe(onFrame)
    else {
      unsubscribe?.()
      unsubscribe = undefined
    }
  })
  observer.observe(element)
  return () => {
    observer.disconnect()
    unsubscribe?.()
  }
}

/** Ticks `onFrame` once per display frame. */
export function subscribeEveryFrame(onFrame: FrameListener) {
  let raf = 0
  let last = performance.now()
  const loop = (now: number) => {
    const dt = Math.min((now - last) / 1000, MAX_DT_SECONDS)
    last = now
    onFrame(dt)
    raf = requestAnimationFrame(loop)
  }
  raf = requestAnimationFrame(loop)
  return () => cancelAnimationFrame(raf)
}
