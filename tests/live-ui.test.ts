import { describe, it, expect, vi } from 'vitest'
import { readFileSync } from 'node:fs'
import { applyCatalog, validateCatalog, readCatalog, LIVE_UI_LIMIT, checkLiveUI } from '../src/live-ui'
import { providers } from '../src/api'
import { guides } from '../src/services'

describe('live UI data without executing remote code', () => {
  it('accepts the published catalogue and rejects executable/configuration changes', () => {
    expect(validateCatalog(JSON.parse(readFileSync(new URL('../live-ui/catalog.json',import.meta.url),'utf8'))).schema).toBe(1)
    for (const value of [null,{schema:2,revision:1},{schema:1,revision:-1},{schema:1,revision:1,script:'alert(1)'},{schema:1,revision:1,ui:{accentDark:'url(https://evil.test)'}},{schema:1,revision:1,services:{drive:{endpoint:'https://evil.test'}}},{schema:1,revision:1,services:{drive:{guide:{fields:[{key:'token'}]}}}}]) expect(()=>validateCatalog(value)).toThrow()
  })
  it('updates only presentation and preserves authentication fields and provider identity', () => {
    const provider=providers.find(p=>p.id==='delivery')!
    const fields=guides.delivery.fields
    expect(applyCatalog(validateCatalog({schema:1,revision:100,services:{delivery:{name:'Delivery live',guide:{intro:'Updated help'}}}}))).toBe(true)
    expect(provider.name).toBe('Delivery live')
    expect(provider.id).toBe('delivery')
    expect(guides.delivery.intro).toBe('Updated help')
    expect(guides.delivery.fields).toEqual(fields)
    expect(applyCatalog(validateCatalog({schema:1,revision:99}))).toBe(false)
    expect(applyCatalog(validateCatalog({schema:1,revision:101}))).toBe(true)
    expect(provider.name).toBe('Delivery')
  })
  it('bounds actual response bytes and rejects malformed or failed responses', async () => {
    await expect(readCatalog(new Response(JSON.stringify({schema:1,revision:2})))).resolves.toMatchObject({revision:2})
    await expect(readCatalog(new Response('a'.repeat(LIVE_UI_LIMIT+1)))).rejects.toThrow('большой')
    await expect(readCatalog(new Response('{'))).rejects.toThrow()
    await expect(readCatalog(new Response('',{status:500}))).rejects.toThrow()
  })
  it('deduplicates requests and omits cookies and referrers', async () => {
    const fetch=vi.fn().mockResolvedValue(new Response(JSON.stringify({schema:1,revision:102})))
    vi.stubGlobal('fetch',fetch)
    try {
      await Promise.all([checkLiveUI(),checkLiveUI()])
      expect(fetch).toHaveBeenCalledTimes(1)
      expect(fetch.mock.calls[0][1]).toMatchObject({credentials:'omit',redirect:'error',referrerPolicy:'no-referrer'})
    } finally { vi.unstubAllGlobals() }
  })
})
