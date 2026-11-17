import { act, renderHook } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import { useSlice } from '../../app/mirror'
import { useChoice } from '../../app/useChoice'
import type { Committed, PillTheme } from '../../shared/bindings'
import { mirror, pillAt, settingsAt, type Send } from '../utils'

const useTheme = () => useSlice('settings', settings => settings.pill.theme)

describe('mirror', () => {
  it('ignores a patch older than the one it holds, and starts over when the window subscribes again', async () => {
    const send = await mirror([settingsAt(0)])
    const { result } = renderHook(useTheme)
    await send(pillAt(2, { theme: 'dawn' }))
    await send(pillAt(1, { theme: 'aurora' }))
    expect(result.current).toBe('dawn')

    await act(async () => { await mirror([pillAt(0, { theme: 'glass' })]) })
    expect(result.current).toBe('glass')
  })
})

describe('useChoice', () => {
  /** Saves Rust answers by hand, in the order they were made. */
  const writes: { value: PillTheme; commit: (rev: number) => Promise<void>; refuse: () => Promise<void> }[] = []
  let send: Send
  const save = (value: PillTheme) => new Promise<Committed>((resolve, reject) => {
    writes.push({
      value,
      commit: async (rev) => { await act(async () => resolve({ rev })) },
      refuse: async () => { await act(async () => reject({ code: 'io', message: 'disk full' })) },
    })
  })
  const onError = vi.fn()
  // A choice outlives its component, so each test picks a value of its own.
  const renderChoice = (id: string) => renderHook(() => ({ saved: useTheme(), ...useChoice('settings', id, save, onError) }))

  beforeEach(async () => {
    writes.length = 0
    onError.mockReset()
    vi.mocked(invoke).mockReset()
    send = await mirror([settingsAt(0)])
  })

  it('saves one pick at a time, the newest next, and shows it until the mirror holds what it committed', async () => {
    const { result } = renderChoice('newest')
    act(() => { result.current.choose('aurora'); result.current.choose('dawn'); result.current.choose('glass') })
    expect(writes.map(w => w.value)).toEqual(['aurora'])
    await writes[0].commit(1)
    expect(writes.map(w => w.value)).toEqual(['aurora', 'glass'])
    await writes[1].commit(2)
    await send(pillAt(1, { theme: 'aurora' }))
    expect(result.current.pending?.value).toBe('glass')
    await send(pillAt(2, { theme: 'glass' }))
    expect(result.current.pending).toBeNull()
    expect(result.current.saved).toBe('glass')
  })

  it('shows the saved value again when a pick is refused, and says why', async () => {
    const { result } = renderChoice('refused')
    act(() => { result.current.choose('dawn') })
    await writes[0].refuse()
    expect(onError).toHaveBeenCalledWith({ code: 'io', message: 'disk full' })
    expect(result.current.pending).toBeNull()
    expect(result.current.saved).toBe('steel')
  })

  it('says nothing when an older pick fails with a newer one waiting, and saves the newer', async () => {
    const { result } = renderChoice('superseded')
    act(() => { result.current.choose('aurora'); result.current.choose('dawn') })
    await writes[0].refuse()
    expect(onError).not.toHaveBeenCalled()
    expect(result.current.pending?.value).toBe('dawn')
    await writes[1].commit(1)
    await send(pillAt(1, { theme: 'dawn' }))
    expect(result.current.pending).toBeNull()
    expect(result.current.saved).toBe('dawn')
  })

  it('keeps one queue for a value when its screen closes and reopens mid-save', async () => {
    const first = renderChoice('reopened')
    act(() => { first.result.current.choose('aurora'); first.result.current.choose('dawn') })
    first.unmount()
    const { result } = renderChoice('reopened')
    expect(result.current.pending?.value).toBe('dawn')
    act(() => { result.current.choose('glass') })
    await writes[0].commit(1)
    expect(writes.map(w => w.value)).toEqual(['aurora', 'glass'])
  })
})
