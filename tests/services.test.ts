import { describe, expect, it } from 'vitest'
import { readFileSync } from 'node:fs'
import { providers } from '../src/api'
import { guides, guideUrl, releaseName, connectionSummary } from '../src/services'

describe('service setup catalogue', () => {
  it('keeps basic login short without hiding required credentials', () => {
    const app = readFileSync(new URL('../src/App.tsx', import.meta.url), 'utf8')
    const selectService = app.split('const selectService =')[1].split('const openGuide')[0]
    expect(selectService).toContain('setOptions({})')
    expect(selectService).not.toContain('https://')
    expect(guides.delivery.fields?.find(f => f.key === 'url')?.placeholder).toBe('https://files.example.com/web/')
    expect(guides.delivery.fields?.filter(f => !f.advanced).map(f => f.key)).toEqual(['url','user','pass'])
    expect(guides.delivery.fields?.find(f => f.key === '2fa')?.advanced).toBe(true)
    expect(guides.sftp.fields?.find(f => f.key === 'pass')?.advanced).not.toBe(true)
    for (const provider of providers) {
      expect(connectionSummary(provider.id).length).toBeGreaterThan(10)
      for (const field of guides[provider.id].fields ?? []) if (field.required) expect(field.advanced).not.toBe(true)
    }
    expect(guides.nextcloud.intro).toContain('адреса вашего сайта')
    expect(guides.owncloud.note).toContain('remote.php/webdav/')
  })
  it('has 30 unique services and a dedicated useful guide for each', () => {
    expect(providers).toHaveLength(30)
    expect(new Set(providers.map(p => p.id)).size).toBe(providers.length)
    expect(Object.keys(guides).sort()).toEqual(providers.map(p => p.id).sort())
    for (const provider of providers) {
      const guide = guides[provider.id]
      expect(guide.intro.length).toBeGreaterThan(25)
      expect(guide.steps.length).toBeGreaterThanOrEqual(2)
      expect(new URL(guideUrl(provider.id)).hostname).toBe(provider.id === 'delivery' ? 'docs.sftpgo.com' : 'rclone.org')
      expect(new Set(guide.fields?.map(f => f.key)).size).toBe(guide.fields?.length ?? 0)
    }
  })
  it('keeps every provider and setup field wired to the native allowlist', () => {
    const native = readFileSync(new URL('../src-tauri/src/engine.rs', import.meta.url), 'utf8')
    const allowed = native.split('let allowed = [')[1].split('];')[0]
    const validKeys = native.split('let valid_keys = [')[1].split('];')[0]
    for (const provider of providers) {
      expect(allowed).toContain(`"${provider.id}"`)
      for (const field of guides[provider.id].fields ?? []) expect(validKeys).toContain(`"${field.key}"`)
    }
    expect(native).toContain('"--obscure"')
  })
  it('masks manually entered credentials and preserves an alphabetic release name', () => {
    for (const guide of Object.values(guides)) {
      for (const field of guide.fields ?? []) {
        if (['pass','password','key','api_key','access_grant','library_key','key_file_pass','2fa','mailbox_password','secret_access_key'].includes(field.key)) expect(field.secret).toBe(true)
      }
    }
    expect(releaseName).toMatch(/^[a-z]+$/i)
  })
})
