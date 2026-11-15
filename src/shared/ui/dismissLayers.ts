type Layer = {
  escape: () => void
  tab?: (event: KeyboardEvent) => void
}

const layers: Layer[] = []

function onKey(event: KeyboardEvent) {
  if (event.defaultPrevented) return
  if (event.key === 'Escape') {
    event.preventDefault()
    event.stopPropagation()
    if (!event.repeat) layers.at(-1)?.escape()
  } else if (event.key === 'Tab') {
    for (let i = layers.length - 1; i >= 0; i -= 1) {
      const tab = layers[i].tab
      if (tab) { tab(event); break }
    }
  }
}

/** Escape goes to the newest open layer, including portals outside its parent. */
export function registerLayer(layer: Layer) {
  if (!layers.length) document.addEventListener('keydown', onKey)
  layers.push(layer)
  return () => {
    layers.splice(layers.indexOf(layer), 1)
    if (!layers.length) document.removeEventListener('keydown', onKey)
  }
}
