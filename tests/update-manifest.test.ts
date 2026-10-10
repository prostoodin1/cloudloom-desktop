import { describe, it, expect } from 'vitest'
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
// @ts-expect-error Plain Node release script is intentionally dependency-free.
import { createManifest } from '../scripts/update-manifest.mjs'

describe('release manifest', () => {
  it('requires all three actual packages and signatures before publishing', () => {
    const directory = mkdtempSync(join(tmpdir(), 'cloudloom-manifest-'))
    const args = { directory, repository: 'prostoodin1/cloudloom-desktop', tag: 'cloudloom-kestrel', version: '0.8.0', name: 'Kestrel' }
    try {
      expect(() => createManifest(args)).toThrow()
      const files = ['Cloudloom_Kestrel_Windows_x64_Setup.exe', 'Cloudloom_Kestrel_macOS_Intel.app.tar.gz', 'Cloudloom_Kestrel_macOS_AppleSilicon.app.tar.gz']
      for (const file of files) { writeFileSync(join(directory, file), 'test-only-package'); writeFileSync(join(directory, `${file}.sig`), Buffer.from('test-only-signature'.repeat(20)).toString('base64')) }
      const result = createManifest(args)
      expect(Object.keys(result.platforms)).toEqual(['windows-x86_64', 'darwin-x86_64', 'darwin-aarch64'])
      expect(result.platforms['darwin-aarch64'].url).toContain('/cloudloom-kestrel/Cloudloom_Kestrel_macOS_AppleSilicon.app.tar.gz')
      expect(() => createManifest({ ...args, repository: '../escape' })).toThrow()
      writeFileSync(join(directory, `${files[0]}.sig`), 'not a signature')
      expect(() => createManifest(args)).toThrow()
    } finally { rmSync(directory, { recursive: true }) }
  })
})
