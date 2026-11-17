import { fireEvent, render, screen } from '@testing-library/react'
import { expect, it } from 'vitest'
import { useState } from 'react'
import { Modal } from '../../shared/ui/Modal'
import { Select } from '../../shared/ui/Select'

function Dialog() {
  const [open, setOpen] = useState(false)
  const [name, setName] = useState('')
  return (
    <>
      <button onClick={() => setOpen(true)}>Open dialog</button>
      <Modal open={open} onClose={() => setOpen(false)}>
        <input aria-label="Name" value={name} onChange={e => setName(e.target.value)} />
        <Select aria-label="Language" value={null} options={[{ value: 'en', label: 'English' }]} onChange={() => {}} />
      </Modal>
    </>
  )
}

function openDialog() {
  render(<Dialog />)
  const trigger = screen.getByRole('button', { name: 'Open dialog' })
  trigger.focus()
  fireEvent.click(trigger)
  return trigger
}

it('keeps focus on the input across an edited dialog render and restores the opener on close', () => {
  const trigger = openDialog()
  expect(screen.getByRole('dialog')).toHaveFocus()
  const input = screen.getByRole('textbox', { name: 'Name' })
  input.focus()
  fireEvent.change(input, { target: { value: 'Sample' } })
  expect(input).toHaveFocus()
  fireEvent.keyDown(input, { key: 'Escape' })
  expect(trigger).toHaveFocus()
})

it('closes a portaled Select before its dialog when Escape is pressed', () => {
  const trigger = openDialog()
  const select = screen.getByRole('button', { name: 'Language' })
  fireEvent.click(select)
  fireEvent.keyDown(screen.getByRole('listbox'), { key: 'Escape' })
  expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
  expect(screen.getByRole('dialog')).toBeInTheDocument()
  expect(select).toHaveFocus()
  fireEvent.keyDown(select, { key: 'Escape' })
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  expect(trigger).toHaveFocus()
})
