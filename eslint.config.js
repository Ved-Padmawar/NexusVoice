import js from '@eslint/js'
import globals from 'globals'
import reactHooks from 'eslint-plugin-react-hooks'
import reactRefresh from 'eslint-plugin-react-refresh'
import tseslint from 'typescript-eslint'
import { defineConfig, globalIgnores } from 'eslint/config'

export default defineConfig([
  globalIgnores(['dist', '.cargo-target', 'src/shared/bindings.ts']),
  {
    files: ['**/*.{ts,tsx}'],
    extends: [
      js.configs.recommended,
      tseslint.configs.recommended,
      reactHooks.configs.flat.recommended,
      reactRefresh.configs.vite,
    ],
    languageOptions: {
      ecmaVersion: 2020,
      globals: globals.browser,
    },
  },
  {
    // Rust pushes its state; nothing in a window edits or re-reads it by hand.
    files: ['src/**/*.{ts,tsx}'],
    ignores: ['src/__tests__/**'],
    rules: {
      'no-restricted-syntax': ['error', {
        selector: 'CallExpression[callee.property.name=/^(setQueryData|setQueriesData|invalidateQueries|refetchQueries|cancelQueries|resetQueries|removeQueries)$/]',
        message: "Read Rust state with useSlice; key a History read on useRevision('history'); after a save, await mirrored(slice, rev).",
      }],
    },
  },
])
