import { useEffect, useRef, useState } from 'react'
import { Copy, Link, LoaderCircle, Trash2, X } from 'lucide-react'
import { call, type Entry, type ShareCapabilities, type ShareData } from './api'

export function ShareDialog({ profileId, path, entry, capabilities, onClose }: {
  profileId: string; path: string; entry: Entry; capabilities: ShareCapabilities; onClose: () => void
}) {
  const [password, setPassword] = useState('')
  const [days, setDays] = useState('7')
  const [share, setShare] = useState<ShareData | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [message, setMessage] = useState('')
  const lock = useRef(false)
  const dialog = useRef<HTMLDivElement>(null)
  const linkInput = useRef<HTMLInputElement>(null)
  const closeRef = useRef(onClose)
  useEffect(() => { closeRef.current = onClose }, [onClose])
  useEffect(() => {
    const focus = document.activeElement as HTMLElement | null
    dialog.current?.focus()
    const key = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !lock.current) closeRef.current()
      if (event.key !== 'Tab') return
      const items = dialog.current?.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), select:not(:disabled)')
      if (!items?.length) return
      const first = items[0], last = items[items.length - 1]
      if (event.shiftKey && (document.activeElement === first || document.activeElement === dialog.current)) { event.preventDefault(); last.focus() }
      else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus() }
    }
    window.addEventListener('keydown', key)
    return () => { window.removeEventListener('keydown', key); focus?.focus() }
  }, [])
  const create = async () => {
    if (lock.current || share) return
    lock.current = true; setBusy(true); setError(''); setMessage('')
    try {
      setShare(await call<ShareData>('create_share', { id: profileId, path, password: capabilities.delivery ? password : null, days: capabilities.delivery ? Number(days) : null }))
      setPassword('')
    } catch (e) { setError(String(e)) }
    finally { lock.current = false; setBusy(false) }
  }
  const revoke = async () => {
    if (lock.current || !share?.id || !window.confirm('Отключить эту ссылку? Получатели потеряют доступ.')) return
    lock.current = true; setBusy(true); setError('')
    try { await call('revoke_share', { id: profileId, shareId: share.id }); setShare(null); setMessage('Ссылка отключена') }
    catch (e) { setError(String(e)) }
    finally { lock.current = false; setBusy(false) }
  }
  const copy = async () => {
    if (!share) return
    try {
      if (!navigator.clipboard) throw new Error('Clipboard unavailable')
      await navigator.clipboard.writeText(share.url)
      setError(''); setMessage('Ссылка скопирована')
    } catch {
      linkInput.current?.focus(); linkInput.current?.select()
      setError('Ссылка выделена. Нажмите Ctrl+C / ⌘C, чтобы скопировать.')
    }
  }
  return <div className="modal-backdrop"><div className="share-dialog" role="dialog" aria-modal="true" aria-labelledby="share-title" tabIndex={-1} ref={dialog}>
    <header><div><p className="eyebrow">ОБМЕН ФАЙЛАМИ</p><h2 id="share-title">Ссылка на {entry.isDir ? 'папку' : 'файл'}</h2></div><button className="round" aria-label="Закрыть ссылки" disabled={busy} onClick={onClose}><X size={18}/></button></header>
    <p className="share-name">{entry.name}</p>
    <p>Любой, у кого есть ссылка{capabilities.delivery ? ' и пароль, если он задан' : ''}, сможет получить доступ. Просмотр видео сам по себе не создаёт публичных ссылок.</p>
    {!share && capabilities.delivery && <div className="share-options"><label>Срок действия<select value={days} disabled={busy} onChange={e => setDays(e.target.value)}><option value="1">1 день</option><option value="7">7 дней</option><option value="30">30 дней</option><option value="90">90 дней</option></select></label><label>Пароль (необязательно)<input type="password" autoComplete="new-password" value={password} disabled={busy} maxLength={128} onChange={e => setPassword(e.target.value)}/></label><small>Только чтение выбранного элемента. Сервер может требовать пароль или запрещать обмен.</small></div>}
    {!capabilities.delivery && <p className="description">Сервис создаст новую или вернёт уже существующую ссылку. Срок и пароль здесь не задаются; отозвать доступ можно на сайте сервиса.</p>}
    {share && <p className="description">Закрытие окна не отключает ссылку. Управлять всеми ссылками можно на сайте сервиса. Передайте пароль получателю отдельно.</p>}
    {share && <div className="share-result"><label>Ссылка<input ref={linkInput} aria-label="Ссылка для обмена" readOnly value={share.url} onFocus={e => e.target.select()}/></label>{share.expiresAt && <small>Действует до {new Date(share.expiresAt).toLocaleString('ru')}</small>}<button className="button primary" onClick={() => void copy()}><Copy size={16}/>Копировать ссылку</button>{share.canRevoke && <button className="button subtle danger-text" disabled={busy} onClick={() => void revoke()}><Trash2 size={16}/>Отключить ссылку</button>}</div>}
    {error && <div className="error" role="alert">{error}</div>}{message && <p role="status">{message}</p>}
    <footer>{!share && <button className="button primary" disabled={busy} onClick={() => void create()}>{busy ? <LoaderCircle size={16} className="spin"/> : <Link size={16}/>}Создать публичную ссылку</button>}<button className="button subtle" disabled={busy} onClick={onClose}>Закрыть</button></footer>
  </div></div>
}
