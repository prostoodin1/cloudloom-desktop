import { useEffect, useSyncExternalStore } from 'react'
import { providers } from './api'
import { guides } from './services'
import './live-ui.css'

export const LIVE_UI_URL = 'https://raw.githubusercontent.com/prostoodin1/cloudloom-desktop/live-ui/live-ui/catalog.json'
export const LIVE_UI_LIMIT = 128 * 1024
const cacheKey = 'cloudloom-live-ui-v1'
const defaults = { welcomeTitle: 'Ваше пространство,', welcomeEmphasis: 'без границ.', subtitle: 'Выберите профиль или подключите новое хранилище.', tagline: 'ОДНО ПРИЛОЖЕНИЕ. ВСЕ ВАШИ ОБЛАКА.', accentDark: '#b6c9ad', accentLight: '#43694b' }
type Ui = typeof defaults
type ServicePatch = { name?: string; description?: string; color?: string; guide?: { intro?: string; steps?: string[]; note?: string } }
export interface LiveCatalog { schema: 1; revision: number; ui?: Partial<Ui>; services?: Record<string, ServicePatch> }
const originalProviders = structuredClone(providers)
const originalGuides = structuredClone(guides)
let snapshot = { ui: defaults, revision: 0, status: 'Встроенный интерфейс', checking: false }
const listeners = new Set<() => void>()
const emit = () => listeners.forEach(fn => fn())
const record = (value: unknown): value is Record<string, unknown> => !!value && typeof value === 'object' && !Array.isArray(value)
const string = (value: unknown, limit: number): value is string => typeof value === 'string' && value.length > 0 && value.length <= limit
const keys = (value: Record<string, unknown>, allowed: string[]) => Object.keys(value).every(key => allowed.includes(key))

// Declarative data only: no HTML, JavaScript, URLs, credentials, adapter changes or new input fields.
export function validateCatalog(value: unknown): LiveCatalog {
  if (!record(value) || !keys(value, ['schema','revision','ui','services']) || value.schema !== 1 || !Number.isSafeInteger(value.revision) || (value.revision as number) < 1) throw Error('Неверная версия каталога')
  if (value.ui !== undefined) {
    if (!record(value.ui) || !keys(value.ui, Object.keys(defaults))) throw Error('Неизвестная настройка интерфейса')
    for (const [key, text] of Object.entries(value.ui)) {
      if (!string(text, 500) || (key.startsWith('accent') && !/^#[\da-f]{6}$/i.test(text))) throw Error('Неверное оформление')
    }
  }
  if (value.services !== undefined) {
    if (!record(value.services) || Object.keys(value.services).length > 150) throw Error('Неверный список сервисов')
    for (const [id, patch] of Object.entries(value.services)) {
      if (!/^[a-z0-9 -]{1,60}$/.test(id) || !record(patch) || !keys(patch, ['name','description','color','guide'])) throw Error('Недопустимое изменение сервиса')
      if (patch.name !== undefined && !string(patch.name, 100)) throw Error('Неверное название')
      if (patch.description !== undefined && !string(patch.description, 300)) throw Error('Неверное описание')
      if (patch.color !== undefined && (typeof patch.color !== 'string' || !/^#[\da-f]{6}$/i.test(patch.color))) throw Error('Неверный цвет')
      if (patch.guide !== undefined) {
        if (!record(patch.guide) || !keys(patch.guide, ['intro','steps','note'])) throw Error('Неверная инструкция')
        for (const key of ['intro','note']) if (patch.guide[key] !== undefined && !string(patch.guide[key], 3000)) throw Error('Слишком длинная инструкция')
        if (patch.guide.steps !== undefined && (!Array.isArray(patch.guide.steps) || patch.guide.steps.length < 2 || patch.guide.steps.length > 20 || !patch.guide.steps.every(text => string(text,3000)))) throw Error('Неверные шаги')
      }
    }
  }
  return value as unknown as LiveCatalog
}

export function applyCatalog(catalog: LiveCatalog) {
  if (catalog.revision <= snapshot.revision) return false
  for (const provider of providers) {
    const original = originalProviders.find(item => item.id === provider.id)!
    const patch = catalog.services?.[provider.id]
    Object.assign(provider, original, patch && { name: patch.name ?? original.name, description: patch.description ?? original.description, color: patch.color ?? original.color })
    guides[provider.id] = { ...originalGuides[provider.id], ...patch?.guide }
  }
  snapshot = { ...snapshot, ui: { ...defaults, ...catalog.ui }, revision: catalog.revision, status: 'Интерфейс обновлён без перезапуска' }
  emit()
  return true
}

export async function readCatalog(response: Response) {
  if (!response.ok || !response.body) throw Error('Каталог недоступен')
  const reader = response.body.getReader()
  const chunks: Uint8Array[] = []
  let count = 0
  try {
    while (true) {
      const { done, value } = await reader.read()
      if (done) break
      count += value.byteLength
      if (count > LIVE_UI_LIMIT) throw Error('Каталог слишком большой')
      chunks.push(value)
    }
  } finally { await reader.cancel().catch(() => {}) }
  const bytes = new Uint8Array(count)
  let offset = 0
  for (const chunk of chunks) { bytes.set(chunk,offset); offset += chunk.byteLength }
  return validateCatalog(JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(bytes)))
}

let running: Promise<void> | null = null
export function checkLiveUI() {
  if (running) return running
  snapshot = { ...snapshot, checking: true }; emit()
  running = (async () => {
    const controller = new AbortController()
    const timer = setTimeout(() => controller.abort(), 8000)
    try {
      const response = await fetch(`${LIVE_UI_URL}?minute=${Math.floor(Date.now()/60000)}`, { signal:controller.signal, cache:'no-store', credentials:'omit', redirect:'error', referrerPolicy:'no-referrer' })
      const catalog = await readCatalog(response)
      if (applyCatalog(catalog)) { try { localStorage.setItem(cacheKey,JSON.stringify(catalog)) } catch { /* storage may be full */ } }
      snapshot = { ...snapshot, status:'Интерфейс актуален' }
    } catch { snapshot = { ...snapshot, status:'Офлайн · сохранённый интерфейс доступен' } }
    finally { clearTimeout(timer); snapshot = { ...snapshot, checking:false }; running = null; emit() }
  })()
  return running
}

export function useLiveUI() {
  const state = useSyncExternalStore(fn => { listeners.add(fn); return () => { listeners.delete(fn) } }, () => snapshot)
  useEffect(() => {
    try { const saved = localStorage.getItem(cacheKey); if (saved && saved.length <= LIVE_UI_LIMIT) applyCatalog(validateCatalog(JSON.parse(saved))) } catch { /* invalid cache falls back to bundled data */ }
    void checkLiveUI()
    const timer = setInterval(() => { if (!document.hidden) void checkLiveUI() },60000)
    const visible = () => { if (!document.hidden) void checkLiveUI() }
    document.addEventListener('visibilitychange',visible)
    return () => { clearInterval(timer); document.removeEventListener('visibilitychange',visible) }
  }, [])
  return { ...state, check: checkLiveUI }
}
