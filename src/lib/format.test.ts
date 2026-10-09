import { describe, expect, it } from 'vitest'
import { formatBytes } from './format'

describe('formatBytes', () => {
  it('formats empty, small and studio-scale values', () => {
    expect(formatBytes(null)).toBe('—')
    expect(formatBytes(-1)).toBe('—')
    expect(formatBytes(0)).toBe('0 B')
    expect(formatBytes(2_840_000_000)).toBe('2.8 GB')
  })
})
