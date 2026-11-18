import { useCallback } from 'react'
import { useNavigate } from 'react-router'
import { useAppStore } from './store/useAppStore'
import { ROUTES, type SettingsTab } from './routes'

/** Opens Settings on `tab`. The tab is chosen first, so Settings renders on it. */
export function useOpenSettings() {
  const navigate = useNavigate()
  return useCallback((tab: SettingsTab) => {
    useAppStore.getState().setActiveSettingsTab(tab)
    void navigate(ROUTES.SETTINGS)
  }, [navigate])
}
