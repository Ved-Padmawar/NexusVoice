import { describe, it, expect } from 'vitest'
import { detectPlatform } from '../../shared/platform'

describe('detectPlatform', () => {
  it('recognises macOS and iOS user agents', () => {
    expect(detectPlatform('Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)')).toBe('macos')
    expect(detectPlatform('Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X)')).toBe('macos')
    expect(detectPlatform('Mozilla/5.0 (iPad; CPU OS 17_0)')).toBe('macos')
  })

  it('recognises a Windows WebView2 user agent', () => {
    // The app's primary platform: Edge WebView2 on Windows 11.
    expect(detectPlatform('Mozilla/5.0 (Windows NT 10.0; Win64; x64) Edg/120.0')).toBe('windows')
  })

  it('falls back to linux for anything else', () => {
    // WebKitGTK reports X11/Linux; an unknown UA should behave like Linux too,
    // which is the conservative default for key labelling.
    expect(detectPlatform('Mozilla/5.0 (X11; Linux x86_64) WebKitGTK')).toBe('linux')
    expect(detectPlatform('something else entirely')).toBe('linux')
  })

  it('checks macOS before Windows so "Mac" never reads as Windows', () => {
    // A UA naming both must resolve to macOS; the order of the checks is what
    // decides, and swapping them would mislabel the Command key as Win.
    expect(detectPlatform('Macintosh; Windows-like shell')).toBe('macos')
  })

})
