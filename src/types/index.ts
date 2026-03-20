import { z } from 'zod'

export const UserSchema = z.object({
  id: z.number(),
  email: z.string(),
})

export const TranscriptSchema = z.object({
  id: z.number(),
  content: z.string(),
  wordCount: z.number(),
  durationSeconds: z.number().nullable(),
  createdAt: z.string(),
})

export const DictionaryEntrySchema = z.object({
  id: z.number(),
  term: z.string(),
  replacement: z.string(),
  hits: z.number(),
  createdAt: z.string(),
})

export const UsageStatsSchema = z.object({
  totalWords: z.number(),
  speakingTimeSeconds: z.number(),
  totalSessions: z.number(),
  avgPaceWpm: z.number(),
})

export const TokenPairResponseSchema = z.object({
  accessToken: z.string(),
  refreshToken: z.string(),
  expiresInSeconds: z.number(),
})

export const AuthResponseSchema = z.object({
  user: UserSchema,
  tokens: TokenPairResponseSchema,
})

export const AuthStateSchema = z.object({
  authenticated: z.boolean(),
  userId: z.number().nullable(),
})

export type User = z.infer<typeof UserSchema>
export type Transcript = z.infer<typeof TranscriptSchema>
export type DictionaryEntry = z.infer<typeof DictionaryEntrySchema>
export type UsageStats = z.infer<typeof UsageStatsSchema>
