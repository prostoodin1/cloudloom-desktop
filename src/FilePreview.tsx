import { useCallback, useEffect, useRef, useState } from 'react'
import { convertFileSrc } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { Download, LoaderCircle, Save, X } from 'lucide-react'
import { call, desktop, type Entry, type PreviewData } from './api'
import { formatBytes } from './lib/format'

export function FilePreview({ profileId, remotePath, entry, onClose, onSaved, onDownload }: {
  profileId: string; remotePath: string; entry: Entry; onClose: () => void; onSaved: () => void; onDownload: () => void
}) {
  const [data, setData] = useState<PreviewData | null>(null)
  const [text, setText] = useState('')
  const [baseline, setBaseline] = useState('')
  const [error, setError] = useState('')
  const [saving, setSaving] = useState(false)
  const [loading, setLoading] = useState(true)
  const [saved, setSaved] = useState(false)
  const dialog = useRef<HTMLDivElement>(null)
  const saveLock = useRef(false)
  const dirty = data?.kind === 'text' && text !== baseline
  const state = useRef({ dirty, saving })
  useEffect(() => { state.current = { dirty, saving } }, [dirty, saving])
  useEffect(() => {
    let disposed = false
    let token: string | undefined
    const focus = document.activeElement as HTMLElement | null
    dialog.current?.focus()
    void call<PreviewData>('prepare_preview', { id: profileId, path: remotePath }).then(result => {
      token = result.token
      if (disposed) { void call('close_preview', { token }).catch(() => {}); return }
      setData(result); setText(result.text ?? ''); setBaseline(result.text ?? '')
    }).catch(e => { if (!disposed) setError(String(e)) }).finally(() => { if (!disposed) setLoading(false) })
    return () => { disposed = true; if (token) void call('close_preview', { token }).catch(() => {}); focus?.focus() }
  }, [profileId, remotePath])
  const close = useCallback(() => {
    if (saveLock.current) return
    if (!dirty || window.confirm('Закрыть без сохранения правок?')) onClose()
  }, [dirty, onClose])
  const saveText = useCallback(async () => {
    if (!data || data.kind !== 'text' || !dirty || saveLock.current) return
    saveLock.current = true; setSaving(true); setError(''); setSaved(false)
    const snapshot = text
    try {
      await call('save_preview_text', { token: data.token, text: snapshot })
      setBaseline(snapshot); setSaved(true); onSaved()
    } catch(e) { setError(String(e)) }
    finally { saveLock.current = false; setSaving(false) }
  }, [data, dirty, text, onSaved])
  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 's') { event.preventDefault(); void saveText() }
      if (event.key === 'Escape') { event.preventDefault(); close() }
      if (event.key === 'Tab') {
        const items = dialog.current?.querySelectorAll<HTMLElement>('button:not(:disabled),textarea:not(:disabled),video,audio')
        if (!items?.length) return
        const first = items[0], last = items[items.length - 1]
        if (event.shiftKey && (document.activeElement === first || document.activeElement === dialog.current)) { event.preventDefault(); last.focus() }
        else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus() }
      }
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [close, saveText])
  useEffect(() => {
    if (!desktop) return
    const win = getCurrentWindow()
    const unlisten = win.onCloseRequested(event => {
      if (!state.current.dirty && !state.current.saving) return
      event.preventDefault()
      if (!state.current.saving && window.confirm('Закрыть приложение без сохранения правок?')) void win.destroy()
    })
    return () => { void unlisten.then(fn => fn()) }
  }, [])
  const src = data?.localPath ? convertFileSrc(data.localPath) : undefined
  const mediaError = () => setError('Формат или кодек не поддерживается системой. Скачайте файл и откройте во внешнем приложении.')
  return <div className="modal-backdrop preview-backdrop"><div className="file-preview" role="dialog" aria-modal="true" aria-labelledby="preview-title" tabIndex={-1} ref={dialog}>
    <header className="preview-header"><div><h2 id="preview-title">{entry.name}</h2><small>{formatBytes(entry.size)}{dirty ? ' · Несохранённые правки' : saved ? ' · Сохранено в хранилище' : ''}</small></div><div className="preview-actions">
      <button className="button subtle" onClick={onDownload}><Download size={15}/>Скачать</button>
      {data?.kind === 'text' && <button className="button primary" onClick={() => void saveText()} disabled={!dirty || saving}>{saving ? <LoaderCircle size={15} className="spin"/> : <Save size={15}/>}Сохранить</button>}
      <button className="round" aria-label="Закрыть просмотр" disabled={saving} onClick={close}><X size={18}/></button>
    </div></header>
    {error && <div className="error" role="alert">{error}</div>}
    <div className="preview-body">{loading ? <div className="empty"><LoaderCircle className="spin" size={30}/><h3>Загружаем файл для просмотра</h3><p>Создаётся временная копия. Большое видео может загружаться несколько минут.</p></div> : data?.kind === 'text' ? <textarea className="text-editor" aria-label={`Редактировать ${entry.name}`} spellCheck={false} value={text} disabled={saving} onChange={event => { setText(event.target.value); setSaved(false) }}/> : data?.kind === 'image' ? <img src={src} alt={entry.name} onError={mediaError}/> : data?.kind === 'video' ? <video src={src} controls preload="metadata" onError={mediaError}/> : data?.kind === 'audio' ? <audio src={src} controls preload="metadata" onError={mediaError}/> : <div className="empty"><h3>Просмотр недоступен</h3><p>Можно скачать файл обычным способом.</p></div>}</div>
    <footer className="preview-footer">{data?.kind === 'text' ? 'UTF-8 · до 2 MiB · Ctrl+S / ⌘S · сохранение заменяет файл в хранилище' : 'Временная копия · до 1 GiB · поддержка форматов и кодеков зависит от системы'}</footer>
  </div></div>
}
