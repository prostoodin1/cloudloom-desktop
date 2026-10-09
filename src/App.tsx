import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { open, save } from '@tauri-apps/plugin-dialog'
import { ArrowDownToLine, ArrowLeft, ArrowUpToLine, Check, ChevronRight, CircleHelp, Cloud, Cloudy, Download, ExternalLink, File, FileAudio, FileImage, FileText, Film, Folder, FolderPlus, Grid2X2, HardDrive, LayoutList, LoaderCircle, LogOut, Menu, Moon, MoreHorizontal, Plus, RefreshCw, Search, Settings2, Sun, Trash2, Upload, Users, X } from 'lucide-react'
import { call, desktop, providers, service, type ConnectReply, type Entry, type Profile, type Task } from './api'
import { formatBytes } from './lib/format'
import { guides, guideUrl, releaseName } from './services'
import './App.css'

type Prompt = { kind: 'folder' | 'rename' | 'delete'; entry?: Entry }
function pathJoin(...parts: string[]) { return parts.filter(Boolean).join('/') }
function iconFor(entry: Entry) {
  if (entry.isDir) return Folder
  const ext = entry.name.split('.').pop()?.toLowerCase() ?? ''
  if (['mp4','mov','mkv','avi','webm'].includes(ext)) return Film
  if (['mp3','wav','flac','aif','m4a'].includes(ext)) return FileAudio
  if (['png','jpg','jpeg','webp','svg','gif','heic'].includes(ext)) return FileImage
  if (['pdf','txt','doc','docx','md'].includes(ext)) return FileText
  return File
}
const statusText: Record<string,string> = { queued: 'В очереди', active: 'Передача', complete: 'Готово', cancelled: 'Отменено', error: 'Ошибка', interrupted: 'Прервано' }

export default function App() {
  const [profiles, setProfiles] = useState<Profile[]>([])
  const [profile, setProfile] = useState<Profile | null>(null)
  const [choose, setChoose] = useState(true)
  const [theme, setTheme] = useState(() => localStorage.getItem('cloudloom-theme') ?? 'dark')
  const [view, setView] = useState(() => localStorage.getItem('cloudloom-view') ?? 'list')
  const [path, setPath] = useState('')
  const [entries, setEntries] = useState<Entry[]>([])
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [query, setQuery] = useState('')
  const [sort, setSort] = useState('name')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')
  const [tasks, setTasks] = useState<Task[]>([])
  const [showTransfers, setShowTransfers] = useState(true)
  const [connecting, setConnecting] = useState<string | null>(null)
  const [showHelp, setShowHelp] = useState(false)
  const [serviceQuery, setServiceQuery] = useState('')
  const [reply, setReply] = useState<ConnectReply | null>(null)
  const [busy, setBusy] = useState(false)
  const [formName, setFormName] = useState('')
  const [root, setRoot] = useState('')
  const [options, setOptions] = useState<Record<string,string>>({})
  const [advanced, setAdvanced] = useState(false)
  const [answer, setAnswer] = useState('')
  const [connectError, setConnectError] = useState('')
  const [prompt, setPrompt] = useState<Prompt | null>(null)
  const [text, setText] = useState('')
  const [actionBusy, setActionBusy] = useState(false)
  const [toast, setToast] = useState('')
  const [dragging, setDragging] = useState(false)
  const loadSequence = useRef(0)
  const current = useRef({ profile, path, choose })
  const reloadRef = useRef<() => void>(() => {})
  const uploadRef = useRef<(paths: string[]) => void>(() => {})
  const notify = (message: string) => setToast(message)

  useEffect(() => { document.documentElement.dataset.theme = theme; localStorage.setItem('cloudloom-theme', theme) }, [theme])
  useEffect(() => { localStorage.setItem('cloudloom-view', view) }, [view])
  useEffect(() => { if (!toast) return; const timer = window.setTimeout(() => setToast(''), 4500); return () => clearTimeout(timer) }, [toast])
  useEffect(() => { current.current = { profile, path, choose } }, [profile, path, choose])
  useEffect(() => {
    if (!desktop) return
    call<Profile[]>('list_profiles').then(setProfiles).catch(e => setError(String(e)))
    call<Task[]>('list_tasks').then(setTasks).catch(e => setError(String(e)))
    const unlisten = listen<Task>('transfer', ({ payload }) => {
      setTasks(items => { const found = items.some(t => t.id === payload.id); return found ? items.map(t => t.id === payload.id ? payload : t) : [payload, ...items] })
      if (payload.status === 'complete' && payload.profileId === current.current.profile?.id) reloadRef.current()
    })
    const drop = getCurrentWebview().onDragDropEvent(event => {
      if (event.payload.type === 'over') setDragging(true)
      else { setDragging(false); if (event.payload.type === 'drop' && !current.current.choose) uploadRef.current(event.payload.paths) }
    })
    return () => { void unlisten.then(fn => fn()); void drop.then(fn => fn()) }
  }, [])

  const reload = useCallback(async () => {
    if (!profile) return
    const sequence = ++loadSequence.current
    setLoading(true); setError('')
    try { const files = await call<Entry[]>('list_files', { id: profile.id, path }); if (sequence === loadSequence.current) { setEntries(files); setSelected(new Set()) } }
    catch(e) { if (sequence === loadSequence.current) { setEntries([]); setError(String(e)) } }
    finally { if (sequence === loadSequence.current) setLoading(false) }
  }, [profile, path])
  useEffect(() => { reloadRef.current = () => { void reload() }; const timer = setTimeout(() => { void reload() }, 0); return () => clearTimeout(timer) }, [reload])
  const enter = (value: Profile) => { ++loadSequence.current; setProfile(value); setPath(''); setEntries([]); setSelected(new Set()); setQuery(''); setError(''); setChoose(false) }
  const navigate = (next: string) => { setPath(next); setSelected(new Set()); setQuery('') }
  const visible = useMemo(() => entries.filter(e => e.name.toLocaleLowerCase().includes(query.toLocaleLowerCase())).sort((a,b) => Number(b.isDir) - Number(a.isDir) || (sort === 'size' ? b.size - a.size : sort === 'date' ? b.modified.localeCompare(a.modified) : a.name.localeCompare(b.name))), [entries, query, sort])
  const picked = entries.filter(e => selected.has(e.path))
  const activeTasks = tasks.filter(t => ['active','queued'].includes(t.status))
  const updateOption = (key: string, value: string) => setOptions(o => ({ ...o, [key]: value }))
  const selectService = (id: string) => { setConnecting(id); setShowHelp(false); setFormName(service(id).name); setRoot(''); setOptions({}); setReply(null); setAdvanced(false); setConnectError('') }
  const openGuide = () => {
    if (!connecting) return
    if (desktop) void call('open_service_help', { provider: connecting }).catch(e => setConnectError(String(e)))
    else window.open(guideUrl(connecting), '_blank', 'noopener,noreferrer')
  }

  const receiveConnect = async (result: ConnectReply) => {
    setReply(result)
    if (result.profile) { setProfiles(await call<Profile[]>('list_profiles')); setConnecting(null); setReply(null); notify('Профиль подключён'); enter(result.profile) }
    else setAnswer(typeof result.question?.default === 'boolean' ? String(result.question.default) : String(result.question?.default ?? ''))
  }
  const connect = async () => {
    setBusy(true); setConnectError('')
    try {
      const result = reply?.id ? await call<ConnectReply>('continue_connect', { id: reply.id, answer }) : await call<ConnectReply>('connect_profile', { request: { provider: connecting, name: formName, root, options } })
      await receiveConnect(result)
    } catch(e) { setConnectError(String(e)); setReply(null) }
    finally { setBusy(false) }
  }
  const closeConnect = () => { if (busy) return; if (reply?.id) void call('cancel_connect', { id: reply.id }); setConnecting(null); setReply(null) }
  const disconnect = async (p: Profile) => {
    try { await call('disconnect_profile', { id: p.id }); setProfiles(items => items.filter(item => item.id !== p.id)); if (profile?.id === p.id) { setProfile(null); setEntries([]) }; notify('Профиль отключён. Данные в облаке сохранены.') } catch(e) { setError(String(e)) }
  }
  const toggle = (entry: Entry) => setSelected(items => { const next = new Set(items); if (next.has(entry.path)) next.delete(entry.path); else next.add(entry.path); return next })
  const upload = async (folder: boolean) => {
    try {
      const paths = await open({ multiple: !folder, directory: folder, title: folder ? 'Выберите папку для загрузки' : 'Выберите файлы для загрузки' })
      if (paths) await uploadPaths(Array.isArray(paths) ? paths : [paths], folder)
    } catch(e) { setError(String(e)) }
  }
  const uploadPaths = async (paths: string[], folder?: boolean) => {
    const state = current.current
    if (!state.profile) return
    for (const local of paths) {
      const name = local.replace(/[\\/]$/, '').split(/[\\/]/).pop() ?? ''
      try {
        // Drag/drop paths are stat'ed by the native backend.
        const directory = folder ?? await call<boolean>('local_is_directory', { path: local })
        await call('start_transfer', { id: state.profile.id, direction: 'upload', path: pathJoin(state.path,name), local, directory, name })
        setShowTransfers(true)
      } catch(e) { setError(String(e)) }
    }
  }
  useEffect(() => { uploadRef.current = paths => { void uploadPaths(paths) } })
  const download = async (items: Entry[]) => {
    if (!profile || !items.length) return
    try {
      if (items.length === 1 && !items[0].isDir) {
        const target = await save({ defaultPath: items[0].name, title: 'Сохранить файл' }); if (!target) return
        await call('start_transfer', { id: profile.id, direction: 'download', path: pathJoin(path,items[0].path), local: target, directory: false, name: items[0].name })
      } else {
        const target = await open({ directory: true, title: 'Куда скачать выбранные файлы и папки?' }); if (!target || Array.isArray(target)) return
        for (const item of items) await call('start_transfer', { id: profile.id, direction: 'download', path: pathJoin(path,item.path), local: `${target}/${item.name}`, directory: item.isDir, name: item.name })
      }
      setShowTransfers(true)
    } catch(e) { setError(String(e)) }
  }
  const submitAction = async () => {
    if (!profile || !prompt) return
    setActionBusy(true); setError('')
    try {
      if (prompt.kind === 'folder') await call('create_folder', { id: profile.id, path, name: text.trim() })
      else if (prompt.kind === 'rename' && prompt.entry) await call('rename_item', { id: profile.id, path: pathJoin(path,prompt.entry.path), name: text.trim() })
      else if (prompt.kind === 'delete') for (const item of prompt.entry ? [prompt.entry] : picked) await call('delete_item', { id: profile.id, path: pathJoin(path,item.path), directory: item.isDir })
      setPrompt(null); await reload(); notify('Операция выполнена')
    } catch(e) { setError(String(e)) }
    finally { setActionBusy(false) }
  }

  return <div className={`app ${choose ? 'choosing' : ''}`}>
    <div className="workspace" aria-hidden={choose ? true : undefined} inert={choose || undefined}>
      <aside className="sidebar">
        <Brand />
        <p className="section-label">ПРОСТРАНСТВО</p>
        <button className="nav active" onClick={() => setShowTransfers(false)}><HardDrive size={18}/>Мои файлы</button>
        <button className="nav" onClick={() => setShowTransfers(v => !v)}><ArrowUpToLine size={18}/>Передачи{activeTasks.length > 0 && <span className="count">{activeTasks.length}</span>}</button>
        <p className="section-label profile-label">МОИ ПРОФИЛИ</p>
        {profiles.map(p => <button key={p.id} className={`profile-nav ${profile?.id === p.id ? 'selected' : ''}`} onClick={() => enter(p)}><ServiceIcon id={p.provider}/><span><strong>{p.name}</strong><small>{service(p.provider).name}</small></span><i/></button>)}
        <button className="add-profile" onClick={() => setChoose(true)}><Plus size={16}/>Добавить профиль</button>
        <div className="sidebar-bottom"><div className="privacy"><Cloud size={17}/><span>Ваши файлы — в вашем облаке.<br/>Прямое соединение с хранилищем.</span></div><button className="theme-button" onClick={() => setTheme(t => t === 'dark' ? 'light' : 'dark')}>{theme === 'dark' ? <Sun size={17}/> : <Moon size={17}/>}<span>{theme === 'dark' ? 'Светлая тема' : 'Тёмная тема'}</span><small>{releaseName}</small></button></div>
      </aside>
      <main>
        <header className="topbar"><div className="breadcrumb"><Cloud size={17}/><span>{profile?.name ?? 'Ваше пространство'}</span><ChevronRight size={14}/><strong>{path.split('/').pop() || 'Мои файлы'}</strong></div><div className="header-buttons"><span className="connection-state"><i/>Подключено</span><button className="round" title="Выбрать профиль" aria-label="Выбрать профиль" onClick={() => setChoose(true)}><Menu size={19}/></button></div></header>
        <section className="files-content">
          <div className="files-heading"><div><p className="eyebrow">{profile ? service(profile.provider).name : 'CLOUDLOOM'}</p><h1>{path.split('/').pop() || 'Мои файлы'}<span>{entries.length}</span></h1><p className="description">{profile?.root ? `${profile.name} / ${profile.root}` : 'Всё нужное. В одном пространстве.'}</p></div><div className="file-actions"><button className="button subtle" onClick={() => { setText(''); setPrompt({kind:'folder'}) }} disabled={profile?.provider === 'google cloud storage'} title={profile?.provider === 'google cloud storage' ? 'В GCS папки создаются загрузкой объектов с префиксом' : undefined}><FolderPlus size={17}/>Новая папка</button><details className="upload-menu"><summary className="button primary"><Upload size={17}/>Загрузить<ChevronRight size={13}/></summary><div className="dropdown"><button onClick={() => void upload(false)}><File size={16}/>Файлы</button><button onClick={() => void upload(true)}><Folder size={16}/>Папку целиком</button></div></details></div></div>
          <div className="pathbar"><button disabled={!path} aria-label="На уровень выше" onClick={() => navigate(path.split('/').slice(0,-1).join('/'))}><ArrowLeft size={16}/></button><button onClick={() => navigate('')}><HardDrive size={14}/>Корень</button>{path.split('/').filter(Boolean).map((part,index) => <span key={index}><ChevronRight size={13}/><button onClick={() => navigate(path.split('/').slice(0,index+1).join('/'))}>{part}</button></span>)}</div>
          <div className="toolbar"><label className="search"><Search size={17}/><input placeholder="Поиск в этой папке" value={query} onChange={e => setQuery(e.target.value)}/><kbd>⌕</kbd></label><select aria-label="Сортировка" value={sort} onChange={e => setSort(e.target.value)}><option value="name">По имени</option><option value="date">По дате</option><option value="size">По размеру</option></select><button className="icon-button" title="Обновить" onClick={() => void reload()} disabled={loading}><RefreshCw size={17} className={loading ? 'spin' : ''}/></button><div className="view-buttons"><button aria-label="Список" className={view === 'list' ? 'active' : ''} onClick={() => setView('list')}><LayoutList size={17}/></button><button aria-label="Сетка" className={view === 'grid' ? 'active' : ''} onClick={() => setView('grid')}><Grid2X2 size={17}/></button></div></div>
          {selected.size > 0 && <div className="selection"><span>Выбрано: {selected.size}</span><button onClick={() => void download(picked)}><Download size={15}/>Скачать</button><button className="danger-text" onClick={() => setPrompt({kind:'delete'})}><Trash2 size={15}/>Удалить</button><button aria-label="Снять выделение" onClick={() => setSelected(new Set())}><X size={15}/></button></div>}
          {error && <div className="error" role="alert">{error}<button aria-label="Закрыть ошибку" onClick={() => setError('')}><X size={15}/></button></div>}
          {loading ? <div className="empty"><LoaderCircle className="spin" size={28}/><h3>Открываем ваши файлы</h3><p>Получаем содержимое хранилища…</p></div> : visible.length ? <div className={view === 'grid' ? 'file-grid' : 'file-list'}>
            {view === 'list' && <div className="list-head"><label><input aria-label="Выбрать всё" type="checkbox" checked={visible.length > 0 && visible.every(e => selected.has(e.path))} onChange={e => setSelected(e.target.checked ? new Set(visible.map(item => item.path)) : new Set())}/></label><span>Название</span><span>Изменён</span><span>Размер</span><span/></div>}
            {visible.map(entry => { const Icon = iconFor(entry); return <div className={`file-row ${selected.has(entry.path) ? 'selected' : ''}`} key={entry.path} onDoubleClick={() => entry.isDir ? navigate(pathJoin(path,entry.path)) : void download([entry])} onContextMenu={e => { e.preventDefault(); setSelected(new Set([entry.path])); const menu = e.currentTarget.querySelector('details'); if (menu) menu.open = true }}>
              <label className="file-check"><input type="checkbox" aria-label={`Выбрать ${entry.name}`} checked={selected.has(entry.path)} onChange={() => toggle(entry)}/></label><button className="entry-name" onClick={() => entry.isDir ? navigate(pathJoin(path,entry.path)) : toggle(entry)}><span className={`file-symbol ${entry.isDir ? 'folder' : Icon === Film ? 'video' : Icon === FileAudio ? 'audio' : ''}`}><Icon size={view === 'grid' ? 32 : 21}/></span><strong>{entry.name}</strong></button><span className="file-date">{entry.modified && !entry.modified.startsWith('0001') ? new Date(entry.modified).toLocaleDateString('ru', { day:'numeric', month:'short', year:'numeric' }) : '—'}</span><span className="file-size">{entry.isDir ? 'Папка' : formatBytes(entry.size)}</span><details className="entry-menu"><summary aria-label={`Действия: ${entry.name}`}><MoreHorizontal size={18}/></summary><div className="dropdown"><button onClick={() => void download([entry])}><Download size={15}/>Скачать{entry.isDir ? ' папку' : ''}</button><button onClick={() => { setText(entry.name); setPrompt({kind:'rename',entry}) }}><Settings2 size={15}/>Переименовать</button><button className="danger-text" onClick={() => setPrompt({kind:'delete',entry})}><Trash2 size={15}/>Удалить</button></div></details>
            </div> })}
          </div> : <div className="empty"><div className="empty-icon"><Folder size={34}/></div><h3>{query ? 'Ничего не найдено' : 'Здесь пока пусто'}</h3><p>{query ? 'Попробуйте другой поисковый запрос.' : 'Загрузите файлы или перетащите сюда папку.'}</p>{!query && <button className="button subtle" onClick={() => void upload(false)}><Plus size={16}/>Добавить файлы</button>}</div>}
          <footer className="file-footer"><span>{visible.length} элементов{selected.size ? ` · Выбрано ${selected.size}` : ''}</span><span><Cloud size={13}/>Обновлено с сервиса</span></footer>
        </section>
        {showTransfers && <section className="transfers"><header><span><ArrowUpToLine size={17}/>Передачи {activeTasks.length > 0 && <b>{activeTasks.length}</b>}</span><button aria-label="Скрыть передачи" onClick={() => setShowTransfers(false)}><X size={16}/></button></header>{tasks.length ? <div className="transfer-items">{tasks.map(task => <div className={`transfer ${task.status}`} key={task.id}><div className="transfer-direction">{task.direction === 'upload' ? <ArrowUpToLine size={18}/> : <ArrowDownToLine size={18}/>}</div><div className="transfer-info"><strong>{task.name}</strong><small>{statusText[task.status]}{task.total > 0 && ` · ${formatBytes(task.bytes)} / ${formatBytes(task.total)}`}{task.speed > 0 && ` · ${formatBytes(task.speed)}/с`}</small>{task.status === 'active' && <progress max={task.total || 1} value={task.bytes}/>} {task.error && <p>{task.error}</p>}</div>{['active','queued'].includes(task.status) ? <button title="Отменить передачу" onClick={() => void call('cancel_transfer',{id:task.id}).catch(e => setError(String(e)))}><X size={15}/></button> : task.status === 'complete' ? <Check size={17}/> : null}</div>)}</div> : <div className="no-transfers">Передач пока нет. Загружайте и скачивайте файлы или папки — прогресс появится здесь.</div>}</section>}
      </main>
    </div>

    {choose && <div className="profile-overlay"><div className="ambient ambient-one"/><div className="ambient ambient-two"/><div className="selector-top"><Brand/><button className="round" aria-label="Переключить тему" onClick={() => setTheme(t => t === 'dark' ? 'light' : 'dark')}>{theme === 'dark' ? <Sun size={18}/> : <Moon size={18}/>}</button></div><div className="welcome-title"><span className="welcome-tag"><i/>ОДНО ПРИЛОЖЕНИЕ. ВСЕ ВАШИ ОБЛАКА.</span><h1>Ваше пространство,<br/><em>без границ.</em></h1><p>Выберите профиль или подключите новое хранилище.</p></div>
      <div className="profile-picker" role="dialog" aria-label="Выбор профиля">
        <section className="saved-profiles"><div className="picker-heading"><Users size={17}/><h2>Мои профили</h2><span>{profiles.length}</span></div><p className="picker-description">Продолжите с нужным аккаунтом</p><div className="profile-cards">{profiles.length ? profiles.map(p => <div className="saved-profile" key={p.id}><button className="profile-open" onClick={() => enter(p)}><ServiceIcon id={p.provider}/><span><strong>{p.name}</strong><small>{service(p.provider).name}{p.root ? ` · ${p.root}` : ''}</small></span><ChevronRight size={17}/></button><button className="disconnect" title="Отключить профиль" aria-label={`Отключить ${p.name}`} onClick={() => void disconnect(p)}><LogOut size={13}/></button></div>) : <div className="no-profiles"><div><Cloudy size={31}/></div><h3>Здесь будут ваши профили</h3><p>Подключите сервис справа.<br/>В следующий раз аккаунт будет ждать вас здесь.</p></div>}</div><button className="new-profile" onClick={() => { setServiceQuery(''); document.getElementById('service-search')?.focus() }}><Plus size={19}/><span>Добавить профиль</span></button><div className="picker-security"><span className="lock-dot"/>OAuth-вход или ключи вашего сервиса.<br/>Учётные данные зашифрованы на компьютере.</div></section>
        <section className="available-services" id="services"><div className="picker-heading"><Cloud size={17}/><h2>Подключить хранилище</h2><span>{providers.length}</span></div><label className="service-search"><Search size={14}/><input id="service-search" aria-label="Найти сервис" placeholder="Найти сервис…" value={serviceQuery} onChange={e => setServiceQuery(e.target.value)}/>{serviceQuery && <button aria-label="Сбросить поиск сервисов" onClick={() => setServiceQuery('')}><X size={13}/></button>}</label><div className="services">{providers.filter(p => `${p.name} ${p.description}`.toLocaleLowerCase().includes(serviceQuery.toLocaleLowerCase().trim())).map(p => <button className="service-option" key={p.id} onClick={() => selectService(p.id)}><ServiceIcon id={p.id}/><span><strong>{p.name}</strong><small>{p.description}</small></span><Plus size={17}/></button>)}{!providers.some(p => `${p.name} ${p.description}`.toLocaleLowerCase().includes(serviceQuery.toLocaleLowerCase().trim())) && <p className="service-empty" role="status">Сервис не найден. Попробуйте другое название.</p>}</div></section>
      </div><div className="welcome-footer"><span>Cloudloom {releaseName}</span><span>Файлы не проходят через наши серверы</span></div>{!desktop && <div className="browser-note">Предпросмотр интерфейса. Для входа и работы с файлами запустите приложение для macOS или Windows.</div>}{error && <div className="selector-error error">{error}<button onClick={() => setError('')}><X size={15}/></button></div>}
    </div>}

    {connecting && <div className="modal-backdrop"><div className="modal connect-modal" role="dialog" aria-modal="true" aria-labelledby="connect-title"><button className="modal-close" aria-label="Закрыть подключение" onClick={closeConnect} disabled={busy}><X size={18}/></button><ServiceIcon id={connecting}/><div className="connect-heading"><h2 id="connect-title">{service(connecting).name}</h2><button className="help-toggle" type="button" aria-label={`Помощь с подключением ${service(connecting).name}`} aria-expanded={showHelp} aria-controls="service-help" title="Как получить ключи и войти" onClick={() => setShowHelp(v => !v)}><CircleHelp size={16}/><span>Помощь</span></button></div><p className="modal-subtitle">{busy || reply?.question ? 'Завершите настройку подключения' : 'Добавьте свой профиль в Cloudloom'}</p>
      {showHelp && <section id="service-help" className="service-help" aria-label={`Инструкция для ${service(connecting).name}`}><h3>Как подключить {service(connecting).name}</h3><p>{guides[connecting].intro}</p><ol>{guides[connecting].steps.map(step => <li key={step}>{step}</li>)}</ol>{guides[connecting].note && <p className="help-note">{guides[connecting].note}</p>}<button className="help-link" type="button" onClick={openGuide}>Подробная инструкция rclone<ExternalLink size={13}/></button><small>Откроется в браузере. Никому не передавайте ключи и токены.</small></section>}
      {busy ? <div className="auth-wait"><LoaderCircle size={31} className="spin"/><h3>Подключаем профиль…</h3><p>{service(connecting).oauth ? 'Официальная страница входа откроется в системном браузере. Выберите аккаунт и разрешите доступ. Окно Cloudloom обновится после входа.' : 'Проверяем учётные данные и доступ к хранилищу. Это может занять некоторое время.'}</p></div> : <form onSubmit={e => { e.preventDefault(); void connect() }}>
        {reply?.question ? <div className="question"><p>{reply.error}</p><label>{reply.question.help}{reply.question.examples?.length ? <select value={answer} onChange={e => setAnswer(e.target.value)}>{!reply.question.exclusive && <option value={answer}>{answer || 'По умолчанию'}</option>}{reply.question.examples.map((example,index) => <option key={index} value={example.Value}>{example.Help || example.Value}</option>)}</select> : typeof reply.question.default === 'boolean' ? <select value={answer} onChange={e => setAnswer(e.target.value)}><option value="true">Да</option><option value="false">Нет</option></select> : <input autoFocus type={reply.question.password ? 'password' : 'text'} value={answer} onChange={e => setAnswer(e.target.value)}/>}</label></div> : <>
          <label>Название профиля<input required autoFocus value={formName} onChange={e => setFormName(e.target.value)} placeholder="Например: Рабочий аккаунт"/></label>
          {connecting === 'alias' && <label>Локальная папка<div className="input-with-button"><input required value={root} onChange={e => setRoot(e.target.value)} placeholder="Выберите папку на компьютере"/><button type="button" onClick={() => void open({directory:true}).then(p => typeof p === 'string' && setRoot(p)).catch(e => setConnectError(String(e)))}>Выбрать</button></div></label>}
          {guides[connecting].fields?.map(field => <label key={field.key}>{field.label}<input required={field.required} type={field.secret ? 'password' : 'text'} autoComplete="off" value={options[field.key] ?? ''} placeholder={field.placeholder} onChange={e => updateOption(field.key,e.target.value)}/></label>)}
          {['b2','azureblob','storj','sftp'].includes(connecting) && <label>{connecting === 'b2' || connecting === 'storj' ? 'Бакет (необязательно)' : connecting === 'azureblob' ? 'Контейнер (необязательно)' : 'Папка относительно домашней (необязательно)'}<input value={root} onChange={e => setRoot(e.target.value)} placeholder="Пусто — корень хранилища"/></label>}
          {connecting === 'seafile' && <label>Двухфакторная авторизация<select value={options['2fa'] ?? 'false'} onChange={e => updateOption('2fa',e.target.value)}><option value="false">Не включена</option><option value="true">Включена — запросить код</option></select></label>}
          {connecting === 'google cloud storage' && <><label>Бакет (необязательно)<input value={root} onChange={e => setRoot(e.target.value)} placeholder="my-studio-bucket"/></label><label>Номер проекта<input value={options.project_number ?? ''} onChange={e => updateOption('project_number',e.target.value)} placeholder="Нужен для списка бакетов"/></label></>}
          {connecting === 'webdav' && <><label>Адрес WebDAV<input required type="url" value={options.url ?? ''} onChange={e => updateOption('url',e.target.value)} placeholder="https://cloud.example.com/remote.php/dav/files/user/"/></label><label>Пользователь<input value={options.user ?? ''} onChange={e => updateOption('user',e.target.value)}/></label><label>Пароль приложения<input type="password" value={options.pass ?? ''} onChange={e => updateOption('pass',e.target.value)} autoComplete="off"/></label></>}
          {connecting === 's3' && <><label>Бакет<input required value={root} onChange={e => setRoot(e.target.value)}/></label><label>Endpoint<input value={options.endpoint ?? ''} onChange={e => { updateOption('endpoint',e.target.value); updateOption('provider','Other') }} placeholder="Пусто для Amazon AWS"/></label><label>Регион<input value={options.region ?? ''} onChange={e => updateOption('region',e.target.value)} placeholder="eu-west-1"/></label><label>Access Key ID<input required value={options.access_key_id ?? ''} onChange={e => updateOption('access_key_id',e.target.value)}/></label><label>Secret Access Key<input required type="password" autoComplete="off" value={options.secret_access_key ?? ''} onChange={e => updateOption('secret_access_key',e.target.value)}/></label></>}
          {service(connecting).oauth && <><button className="advanced-toggle" type="button" onClick={() => setAdvanced(v => !v)}><Settings2 size={14}/>Настройки OAuth<ChevronRight size={13}/></button>{connecting === 'drive' && <p className="setup-note">Google переводит приложения на собственные OAuth Client ID. Если стандартный вход недоступен, укажите Client ID и Client Secret вашего Desktop OAuth приложения в настройках ниже.</p>}{advanced && <div className="advanced"><label>OAuth Client ID<input value={options.client_id ?? ''} onChange={e => updateOption('client_id',e.target.value)}/></label><label>OAuth Client Secret (если требуется)<input type="password" autoComplete="off" value={options.client_secret ?? ''} onChange={e => updateOption('client_secret',e.target.value)}/></label><p>Настройки и токены хранятся в зашифрованном виде на этом компьютере.</p></div>}</>}
        </>}
        {connectError && <div className="error" role="alert">{connectError}</div>}
        <button className="button primary connect-submit" type="submit">{reply?.question ? 'Продолжить' : service(connecting).oauth ? 'Войти через браузер' : 'Подключить профиль'}<ChevronRight size={16}/></button>
      </form>}
    </div></div>}
    {prompt && <div className="modal-backdrop"><form className="modal action-modal" role="dialog" aria-modal="true" onSubmit={e => { e.preventDefault(); void submitAction() }}><button type="button" className="modal-close" aria-label="Закрыть диалог" disabled={actionBusy} onClick={() => setPrompt(null)}><X size={18}/></button><h2>{prompt.kind === 'folder' ? 'Новая папка' : prompt.kind === 'rename' ? 'Переименовать' : 'Удалить выбранное?'}</h2>{prompt.kind === 'delete' ? <p className="modal-subtitle">{prompt.entry ? prompt.entry.name : `${picked.length} элементов`}. Папки удаляются вместе со всем содержимым. Восстановление зависит от сервиса.</p> : <label>Название<input autoFocus required value={text} onChange={e => setText(e.target.value)}/></label>}<div className="dialog-actions"><button type="button" className="button subtle" disabled={actionBusy} onClick={() => setPrompt(null)}>Отмена</button><button className={`button ${prompt.kind === 'delete' ? 'danger' : 'primary'}`} disabled={actionBusy || (prompt.kind !== 'delete' && !text.trim())}>{actionBusy && <LoaderCircle size={15} className="spin"/>}{prompt.kind === 'delete' ? 'Удалить' : 'Сохранить'}</button></div>{error && <p className="error">{error}</p>}</form></div>}
    {dragging && !choose && <div className="drop-overlay"><Upload size={40}/><h2>Отпустите, чтобы загрузить</h2><p>Файлы и папки попадут в текущую папку</p></div>}
    {toast && <div className="toast" role="status"><Check size={16}/>{toast}</div>}
  </div>
}

function Brand() { return <div className="brand"><span className="brand-logo"><Cloudy size={23}/></span><strong>cloudloom<span>®</span></strong></div> }
function ServiceIcon({id}:{id:string}) { const p = service(id); return <span className={`service-icon service-${id.replaceAll(' ','-')}`} style={{'--service':p.color} as React.CSSProperties}>{id === 'drive' ? <svg width="24" height="24" viewBox="0 0 24 24" aria-hidden="true"><path fill="#25a65a" d="M8 2h8L5 21 1 14z"/><path fill="#f6c344" d="M8 2h8l8 14h-8z"/><path fill="#4285f4" d="M5 21l4-7h15l-4 7z"/></svg> : id === 'onedrive' ? <Cloud size={25} fill="currentColor"/> : id === 'alias' ? <HardDrive size={22}/> : p.letters}</span> }
