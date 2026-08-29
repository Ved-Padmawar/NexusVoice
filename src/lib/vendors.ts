import type { ComponentType, SVGProps } from 'react'
import openai from '../assets/providers/openai.svg?react'
import anthropic from '../assets/providers/anthropic.svg?react'
import ollama from '../assets/providers/ollama.svg?react'
import lmstudio from '../assets/providers/lmstudio.svg?react'
import openrouter from '../assets/providers/openrouter.svg?react'
import nvidia from '../assets/providers/nvidia.svg?react'
import qwen from '../assets/providers/qwen.svg?react'
import moonshinePng from '../assets/providers/moonshine.png'

/**
 * The single registry of vendor logos.
 *
 * Both the LLM provider picker and the model catalog resolve through here, so a
 * logo is declared once. All marks are real vendor files inlined at build time —
 * nothing is fetched at runtime, which the app's CSP would block anyway.
 */

type Vendor = {
  Mark: ComponentType<SVGProps<SVGSVGElement>>
  /** Brand colour, or `null` when the file carries its own (Claude, Qwen). */
  color: string | null
}

export const VENDORS = {
  openai: { Mark: openai, color: '#10A37F' },
  anthropic: { Mark: anthropic, color: null },
  ollama: { Mark: ollama, color: '#FFFFFF' },
  lmstudio: { Mark: lmstudio, color: '#4B27E5' },
  openrouter: { Mark: openrouter, color: null },
  nvidia: { Mark: nvidia, color: '#76B900' },
  qwen: { Mark: qwen, color: null },
} as const satisfies Record<string, Vendor>

export type VendorId = keyof typeof VENDORS

/** Moonshine publishes only a raster logo, so it renders as an image. */
export const RASTER = { moonshine: moonshinePng } as const

export type RasterId = keyof typeof RASTER

export const isRaster = (v: string): v is RasterId => v in RASTER

/** Which vendor trained each model family. */
const FAMILY_VENDOR: Record<string, VendorId | RasterId> = {
  whisper: 'openai',
  parakeet: 'nvidia',
  nemotron: 'nvidia',
  canary: 'nvidia',
  qwen3asr: 'qwen',
  moonshine: 'moonshine',
}

export function vendorForFamily(family: string): VendorId | RasterId | null {
  return FAMILY_VENDOR[family.replace(/-/g, '')] ?? null
}
