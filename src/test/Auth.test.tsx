import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { Auth } from '../pages/Auth'
import { useAppStore } from '../store/useAppStore'

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: vi.fn(() => ({
    minimize: vi.fn(),
    toggleMaximize: vi.fn(),
    close: vi.fn(),
  })),
}))

const mockLogin = vi.fn()
const mockRegister = vi.fn()

beforeEach(() => {
  mockLogin.mockReset()
  mockRegister.mockReset()
  useAppStore.setState({ user: null, authChecking: false })
  useAppStore.setState({ login: mockLogin, register: mockRegister } as never)
})

function renderAuth() {
  return render(
    <MemoryRouter>
      <Auth />
    </MemoryRouter>
  )
}

describe('Auth — login mode', () => {
  it('renders sign in form by default', () => {
    renderAuth()
    expect(screen.getByText('Welcome back')).toBeInTheDocument()
    expect(screen.getByLabelText(/email/i)).toBeInTheDocument()
    expect(screen.getByLabelText(/password/i)).toBeInTheDocument()
  })

  it('shows validation error for invalid email', async () => {
    renderAuth()
    fireEvent.input(screen.getByLabelText(/email/i), { target: { value: 'notanemail' } })
    fireEvent.submit(screen.getByRole('button', { name: /sign in/i }))
    await waitFor(() => {
      expect(screen.getByText(/valid email/i)).toBeInTheDocument()
    })
  })

  it('calls login with email and password', async () => {
    mockLogin.mockResolvedValue(undefined)
    renderAuth()
    fireEvent.input(screen.getByLabelText(/email/i), { target: { value: 'test@example.com' } })
    fireEvent.input(screen.getByLabelText(/password/i), { target: { value: 'password123' } })
    fireEvent.submit(screen.getByRole('button', { name: /sign in/i }))
    await waitFor(() => {
      expect(mockLogin).toHaveBeenCalledWith('test@example.com', 'password123')
    })
  })

  it('shows error banner on failed login', async () => {
    mockLogin.mockRejectedValue(new Error('Invalid credentials'))
    renderAuth()
    fireEvent.input(screen.getByLabelText(/email/i), { target: { value: 'test@example.com' } })
    fireEvent.input(screen.getByLabelText(/password/i), { target: { value: 'wrong' } })
    fireEvent.submit(screen.getByRole('button', { name: /sign in/i }))
    await waitFor(() => {
      expect(screen.getByText(/invalid credentials/i)).toBeInTheDocument()
    })
  })
})

describe('Auth — register mode', () => {
  it('switches to register mode', async () => {
    renderAuth()
    fireEvent.click(screen.getByText('Create one'))
    await waitFor(() => {
      expect(screen.getByText('Get started')).toBeInTheDocument()
    })
  })

  it('shows password strength hint in register mode', () => {
    renderAuth()
    fireEvent.click(screen.getByText('Create one'))
    expect(screen.getByText(/min. 8 chars/i)).toBeInTheDocument()
  })

  it('validates password requirements on register', async () => {
    renderAuth()
    fireEvent.click(screen.getByText('Create one'))
    fireEvent.input(screen.getByLabelText(/email/i), { target: { value: 'test@example.com' } })
    fireEvent.input(screen.getByLabelText(/password/i), { target: { value: 'weak' } })
    fireEvent.submit(screen.getByRole('button', { name: /create account/i }))
    await waitFor(() => {
      expect(screen.getByText(/at least 8/i)).toBeInTheDocument()
    })
  })
})
