import { useCallback, useEffect, useImperativeHandle, useRef, useState, type Ref } from 'react'
import { ExternalLink, HardDrive, LoaderCircle, RefreshCw, Unplug, X } from 'lucide-react'
import { call, desktop, service, type Profile } from './api'
import './explorer-disks.css'

export interface DiskOptions { id: string; letter: string; name: string; readOnly: boolean; autoMount: boolean; serviceIcon: boolean }
interface Disk { options: DiskOptions; mounted: boolean; pending: number | null; error: string | null }
interface Overview { windows: boolean; winFsp: boolean; letters: string[]; disks: Disk[] }
function defaults(profile: Profile, overview: Overview): DiskOptions {
  return overview.disks.find(d => d.options.id === profile.id)?.options ?? { id: profile.id, letter: overview.letters[0] ?? '', name: `${service(profile.provider).name} — ${profile.name}`.slice(0, 32).trim(), readOnly: true, autoMount: false, serviceIcon: true }
}
export interface ExplorerDisksHandle { selectProfile: (id: string) => void }
export function ExplorerDisks({ profiles, open, onClose, ref }: { profiles: Profile[]; open: boolean; onClose: () => void; ref?: Ref<ExplorerDisksHandle> }) {
  const [overview, setOverview] = useState<Overview | null>(null)
  const [draft, setForm] = useState<DiskOptions | null>(null)
  const [targetId, setTargetId] = useState<string | null>(null)
  useImperativeHandle(ref, () => ({ selectProfile: id => { setTargetId(id); setForm(null) } }), [])
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const [installation, setInstallation] = useState('')
  const [restartRequired, setRestartRequired] = useState(false)
  const lock = useRef(false)
  const polling = useRef(false)
  const dialog = useRef<HTMLDivElement>(null)
  const closeRef = useRef(onClose)
  const target = profiles.find(p => p.id === targetId) ?? profiles[0]
  const form = draft && profiles.some(p => p.id === draft.id) ? draft : target && overview ? defaults(target, overview) : null
  useEffect(() => { closeRef.current = onClose }, [onClose])
  const refresh = useCallback(async () => {
    if (!desktop || polling.current) return
    polling.current = true
    try {
      const result = await call<Overview>('explorer_disks')
      if (!result || typeof result.windows !== 'boolean' || !Array.isArray(result.disks) || !Array.isArray(result.letters)) throw new Error('Не удалось прочитать состояние дисков. Перезапустите Cloudloom.')
      setOverview(result)
    }
    catch (e) { setError(String(e)) }
    finally { polling.current = false }
  }, [])
  useEffect(() => {
    void Promise.resolve().then(refresh)
    const timer = window.setInterval(() => { if (!lock.current) void refresh() }, 5000)
    return () => clearInterval(timer)
  }, [refresh])
  useEffect(() => {
    if (!open) return
    const focus = document.activeElement as HTMLElement | null
    dialog.current?.focus()
    const key = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && !lock.current) { e.preventDefault(); closeRef.current() }
      if (e.key !== 'Tab') return
      const items = dialog.current?.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), select:not(:disabled), a[href]')
      if (!items?.length) return
      const first = items[0], last = items[items.length - 1]
      if (e.shiftKey && (document.activeElement === first || document.activeElement === dialog.current)) { e.preventDefault(); last.focus() }
      else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first.focus() }
    }
    window.addEventListener('keydown', key)
    return () => { window.removeEventListener('keydown', key); focus?.focus() }
  }, [open])
  if (!open) return null
  const profile = profiles.find(p => p.id === form?.id)
  const disk = overview?.disks.find(d => d.options.id === form?.id)
  const mounted = disk?.mounted ?? false
  const unsupported = profile?.transport === 'sftpgo-api'
  const set = (patch: Partial<DiskOptions>) => { if (form) setForm({ ...form, ...patch }) }
  const action = async (command: 'connect_explorer' | 'disconnect_explorer' | 'open_explorer') => {
    if (!form || lock.current) return
    if (command === 'disconnect_explorer' && !window.confirm('Закройте все файлы этого диска в других программах. Отключить диск? Неотправленные изменения блокируют отключение. Автоподключение будет выключено.')) return
    lock.current = true; setBusy(true); setError('')
    try {
      await call(command, command === 'connect_explorer' ? { options: form } : { id: form.id })
      if (command === 'disconnect_explorer') setForm({ ...form, autoMount: false })
    }
    catch (e) { setError(String(e)) }
    finally { await refresh(); lock.current = false; setBusy(false) }
  }
  const install = async () => {
    if (lock.current || !window.confirm('Установить WinFsp из пакета Cloudloom? Откроется официальный установщик драйвера. Windows запросит права администратора; подтвердите установку и лицензию в его окне.')) return
    lock.current = true; setBusy(true); setError(''); setInstallation('Завершите установку WinFsp в окне Windows…')
    try {
      const result = await call<string>('install_winfsp')
      setRestartRequired(result === 'restartRequired')
      setInstallation(result === 'restartRequired' ? 'WinFsp установлен. Перед подключением диска перезагрузите Windows.' : 'WinFsp установлен. Теперь можно подключить диск.')
    } catch (e) { setInstallation(''); setError(String(e)) }
    finally { await refresh(); lock.current = false; setBusy(false) }
  }
  return <div className="modal-backdrop"><div ref={dialog} className="explorer-dialog" role="dialog" aria-modal="true" aria-labelledby="explorer-title" tabIndex={-1}>
    <header><div><p className="eyebrow">ВАШИ ОБЛАКА В WINDOWS</p><h2 id="explorer-title"><HardDrive size={23}/>Диски в Проводнике</h2></div><button className="round" aria-label="Закрыть диски" disabled={busy} onClick={onClose}><X size={18}/></button></header>
    <p className="explorer-intro">Облако появится как отдельный диск. Название и значок меняются только на этом компьютере, не в облачном аккаунте.</p>
    <p className="explorer-credits">Cloudloom · MIT. WinFsp - Windows File System Proxy, Copyright (C) Bill Zissimopoulos. WinFsp распространяется под GPLv3 с FLOSS-исключением. <button type="button" onClick={() => { void call('open_winfsp_help').catch(e => setError(String(e))) }}>github.com/winfsp/winfsp — исходники и лицензия</button></p>
    {!desktop && <p className="error">Откройте настольное приложение Cloudloom.</p>}
    {overview && !overview.windows && <p className="error">Эта возможность пока доступна только в Windows. Обычная работа с облаками на Mac не меняется.</p>}
    {overview?.windows && !overview.winFsp && <div className="explorer-notice"><strong>Нужен WinFsp</strong><p>Официальный установщик уже включён в Cloudloom — скачивать отдельно не нужно. Установка начнётся только после подтверждения; Windows запросит права администратора. Возможна перезагрузка Windows.</p><button className="button primary" disabled={busy} onClick={() => { void install() }}>{busy ? <LoaderCircle className="spin" size={15}/> : <HardDrive size={15}/>}Установить WinFsp</button></div>}
    {installation && <p role="status" className="explorer-notice">{installation}</p>}
    {!overview && !error && <p role="status"><LoaderCircle className="spin" size={17}/>Проверяем диски…</p>}
    {!profiles.length && <p>Сначала подключите профиль облака или локальной папки в главном меню.</p>}
    {profile && form && overview && <>
      <label className="disk-field">Профиль<select value={profile.id} disabled={busy} onChange={e => { const selected = profiles.find(p => p.id === e.target.value); if (selected) { setForm(defaults(selected, overview)); setError('') } }}>{profiles.map(p => <option key={p.id} value={p.id}>{p.name} · {service(p.provider).name}</option>)}</select></label>
      {unsupported && <div className="explorer-notice">Delivery с входом через API пока не подключается как диск. Если у сервера есть WebDAV, его можно добавить отдельным профилем с адресом, полученным у администратора. Адрес не угадывается и не подставляется.</div>}
      <div className="disk-fields"><label className="disk-field">Буква диска<select value={form.letter} disabled={busy || mounted} onChange={e => set({ letter: e.target.value })}>{[...new Set([form.letter, ...overview.letters])].filter(Boolean).map(letter => <option key={letter} value={letter}>{letter}:</option>)}{!form.letter && <option value="">Нет свободных букв</option>}</select></label><label className="disk-field">Название в Проводнике<input maxLength={32} value={form.name} disabled={busy || mounted} onChange={e => set({ name: e.target.value })}/></label></div>
      <div className="disk-checkboxes"><label><input type="checkbox" checked={form.serviceIcon} disabled={busy || mounted} onChange={e => set({ serviceIcon: e.target.checked })}/>Значок сервиса (логотип или цветной буквенный значок)</label><label><input type="checkbox" checked={form.readOnly} disabled={busy || mounted} onChange={e => set({ readOnly: e.target.checked })}/>Только чтение — без изменения и удаления облачных файлов</label><label><input type="checkbox" checked={form.autoMount} disabled={busy || mounted} onChange={e => set({ autoMount: e.target.checked })}/>Подключать при запуске Cloudloom</label></div>
      {!form.readOnly && <p className="explorer-notice">Запись включена: изменение или удаление в Проводнике изменит файлы в облаке. Отправка начинается после закрытия файла. Перед отключением закройте использующие диск программы и дождитесь отправки.</p>}
      <p className="explorer-cache">Диск работает, пока открыт Cloudloom. Кэш хранится локально без дополнительного шифрования: чтение по частям, запись через кэш. Целевой размер — 10 ГБ, резерв — 1 ГБ; открытые и неотправленные файлы могут превысить лимит. Это не полностью офлайн-копия и не защита от конфликтов между программами.</p>
      {disk?.mounted && <div className="disk-state" role="status"><strong>{disk.options.letter}: · {disk.options.name}</strong><span>Подключён · очередь/ошибки: {disk.pending ?? 'неизвестно'}</span><small>Чтобы переименовать, сменить букву или режим, отключите диск и подключите с новыми настройками.</small></div>}
      {disk?.error && <p className="error" role="alert">{disk.error}</p>}
      <footer>{mounted ? <><button className="button primary" disabled={busy} onClick={() => { void action('open_explorer') }}><ExternalLink size={16}/>Открыть Проводник</button><button className="button subtle" disabled={busy} onClick={() => { void action('disconnect_explorer') }}><Unplug size={16}/>Отключить диск</button></> : <button className="button primary" disabled={busy || !overview.windows || !overview.winFsp || restartRequired || unsupported || !form.letter || !form.name.trim()} onClick={() => { void action('connect_explorer') }}>{busy ? <LoaderCircle className="spin" size={16}/> : <HardDrive size={16}/>}Подключить диск</button>}<button className="button subtle" aria-label="Обновить диски" disabled={busy} onClick={() => { void refresh() }}><RefreshCw size={16}/></button></footer>
    </>}
    {error && <p className="error" role="alert">{error}</p>}
  </div></div>
}
