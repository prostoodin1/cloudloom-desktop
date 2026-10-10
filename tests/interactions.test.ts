import { describe, expect, it } from 'vitest'
import { canDrop } from '../src/useFileInteractions'

describe('file drop targets', () => {
  it('allows folders, files and multi-selection into another directory or parent', () => {
    expect(canDrop(['file.txt', 'photos'], 'archive')).toBe(true)
    expect(canDrop(['archive/file.txt', 'archive/photos'], '')).toBe(true)
    expect(canDrop(['a/b/c'], 'a')).toBe(true)
  })
  it('rejects self, descendants, same parent and empty drags', () => {
    expect(canDrop(['photos'], 'photos')).toBe(false)
    expect(canDrop(['photos'], 'photos/2026')).toBe(false)
    expect(canDrop(['photos', 'file.txt'], '')).toBe(false)
    expect(canDrop(['a/file.txt'], 'a')).toBe(false)
    expect(canDrop([], 'a')).toBe(false)
  })
})
