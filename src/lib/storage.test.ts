import { describe, expect, it } from 'vitest'
import { usageLabel, usagePercent } from './storage'
describe('storage usage', () => {
  it('distinguishes missing quota from empty storage', () => {
    expect(usageLabel()).toBe('Проверяем объём…')
    expect(usageLabel({ used: null, total: null, free: null, message: null })).toBe('Объём не предоставлен')
    expect(usageLabel({ used: 0, total: 15e9, free: 15e9, message: null })).toBe('0 B из 15.0 GB занято')
  })
  it('does not invent an unlimited account total and bounds the meter', () => {
    expect(usagePercent({ used: 20, total: 10, free: 0, message: null })).toBe(100)
    expect(usagePercent({ used: 0, total: 0, free: null, message: null })).toBeNull()
    expect(usagePercent()).toBeNull()
    expect(usageLabel({ used: 50, total: null, free: null, message: null })).toContain('лимит неизвестен')
  })
})
