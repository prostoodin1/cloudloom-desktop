import { useEffect, useState } from 'react'
import { Channel } from '@tauri-apps/api/core'
import { Cloud, LoaderCircle } from 'lucide-react'
import { call, desktop } from './api'
import App from './App'
import './startup-update.css'

type Progress = { phase: 'check' | 'download' | 'verify' | 'install'; downloaded: number; total: number | null }
let startup: Promise<string> | null = null
let latest: Progress = { phase: 'check', downloaded: 0, total: null }
const listeners = new Set<(progress: Progress) => void>()
function begin() {
  if (!startup) {
    const progress = new Channel<Progress>()
    progress.onmessage = value => { latest = value; listeners.forEach(fn => fn(value)) }
    startup = call<string>('startup_update', { progress })
  }
  return startup
}
export default function StartupUpdate() {
  const [ready, setReady] = useState(!desktop)
  const [error, setError] = useState('')
  const [progress, setProgress] = useState(latest)
  useEffect(() => {
    if (!desktop) return
    let disposed = false
    listeners.add(setProgress)
    void begin().then(() => { if (!disposed) setReady(true) }).catch(e => { if (!disposed) setError(String(e)) })
    return () => { disposed = true; listeners.delete(setProgress) }
  }, [])
  if (ready) return <App/>
  const percent = progress.total ? Math.min(100, Math.round(progress.downloaded / progress.total * 100)) : null
  const title = { check: 'Проверяем обновления', download: 'Скачиваем обновление', verify: 'Проверяем подпись', install: 'Устанавливаем обновление' }[progress.phase]
  return <main className="startup-update"><section role="status" aria-live="polite"><div className="startup-logo"><Cloud size={34}/></div><p className="eyebrow">CLOUDLOOM</p><h1>{error ? 'Не удалось обновиться' : title}</h1><p>{error || (progress.phase === 'check' ? 'Перед началом работы проверим новую версию.' : 'Приложение обновится и откроется заново. Профили и файлы остаются на месте.')}</p>{!error && <><LoaderCircle className="spin" size={24}/>{progress.phase === 'download' && <><progress aria-label="Загрузка обновления" max={100} value={percent ?? undefined}/><small>{percent === null ? 'Получаем пакет…' : `${percent}%`}</small></>}</>}{error && <button className="startup-continue" onClick={() => setReady(true)}>Продолжить без обновления</button>}<footer>Только подписанные обновления · ваши облачные файлы не передаются</footer></section></main>
}
