import { QueryClient } from '@tanstack/react-query'

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // These queries use local Tauri IPC, even when the PC is offline.
      networkMode: 'always',
      staleTime: 30_000,
      refetchOnWindowFocus: false,
      retry: false,
    },
    mutations: { networkMode: 'always' },
  },
})
