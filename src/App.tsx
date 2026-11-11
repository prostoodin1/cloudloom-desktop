import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { open, save } from '@tauri-apps/plugin-dialog'
import { ArrowDownToLine, ArrowLeft, ArrowUpToLine, Check, ChevronRight, CircleAlert, CircleHelp, Clock3, Cloud, CloudCheck, Cloudy, Download, ExternalLink, File, FileAudio, FileImage, FileText, Film, Folder, FolderPlus, Grid2X2, HardDrive, LayoutList, LoaderCircle, LogOut, Menu, Moon, MoreHorizontal, Plus, RefreshCw, Search, Settings2, Sun, Trash2, Upload, Users, X } from 'lucide-react'
import { call, desktop, providers, service, type ConnectReply, type Entry, type Profile, type Task, type StorageUsage } from './api'
import { FilePreview } from './FilePreview'
import { ShareDialog } from './ShareDialog'
import { ExplorerDisks, type ExplorerDisksHandle } from './ExplorerDisks'
import type { ShareCapabilities } from './api'
import { useLiveUI } from './live-ui'
import { useChoiceActivation, useFileInteractions } from './useFileInteractions'
import './file-interactions.css'
import { usageLabel, usagePercent } from './lib/storage'
import { formatBytes } from './lib/format'
import { guides, guideUrl, releaseName, connectionSummary } from './services'
import './App.css'

type CloseMode = 'keepOpen' | 'tray' | 'disconnect'
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

type ExplorerTransfer = {
  profileId: string
  letter: string
  path: string
  status: 'queued' | 'active' | 'retrying' | 'error' | 'complete'
  bytes: number
  total: number
  completedAt: string | null
}
const explorerStatus = {
  queued: 'Ожидает отправки',
  active: 'Отправляется',
  retrying: 'Сбой отправки · повтор в очереди',
  error: 'Ошибка отправки',
  complete: 'Отправлено',
}
function explorerHistoryKey(task: ExplorerTransfer) {
  return JSON.stringify([task.profileId, task.path, task.completedAt])
}
function SyncBadge({ status }: { status: ExplorerTransfer['status'] }) {
  const Icon = status === 'complete' ? CloudCheck : status === 'active' ? RefreshCw : status === 'queued' ? Clock3 : CircleAlert
  return <span className={`sync-badge ${status}`} role="img" title={explorerStatus[status]} aria-label={explorerStatus[status]}><Icon size={15}/></span>
}

export default function App() {
  const liveUI = useLiveUI()
  const [profiles, setProfiles] = useState<Profile[]>([])
  const [profile, setProfile] = useState<Profile | null>(null)
  const [choose, setChoose] = useState(true)
  const [generalSettings, setGeneralSettings] = useState(false)
  const [closeMode, setCloseMode] = useState<CloseMode>('keepOpen')
  const [settingsError, setSettingsError] = useState('')
  const [savingSettings, setSavingSettings] = useState(false)
  const openSettings = async () => {
    setSettingsError('')
    try {
      setCloseMode(await call<CloseMode>('background_settings'))
      setGeneralSettings(true)
    } catch (e) { setToast(String(e)) }
  }
  const changeCloseMode = async (mode: CloseMode) => {
    setSavingSettings(true)
    setSettingsError('')
    try {
      await call('save_background_settings', { mode })
      setCloseMode(mode)
    } catch (e) { setSettingsError(String(e)) }
    finally { setSavingSettings(false) }
  }
  const [explorer, setExplorer] = useState(false)
  const explorerPanel = useRef<ExplorerDisksHandle>(null)
  const mountProfile = (id: string) => { explorerPanel.current?.selectProfile(id); setExplorer(true) }
  const [theme, setTheme] = useState(() => localStorage.getItem('cloudloom-theme') ?? 'dark')
  const [view, setView] = useState(() => localStorage.getItem('cloudloom-view') ?? 'list')
  const [path, setPath] = useState('')
  const [entries, setEntries] = useState<Entry[]>([])
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [query, setQuery] = useState('')
  const [sort, setSort] = useState('name')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')
  const [monthlyDownloads, setMonthlyDownloads] = useState<number | null>(null)
  const [tasks, setTasks] = useState<Task[]>([])
  const [showTransfers, setShowTransfers] = useState(false)
  const [explorerTransfers, setExplorerTransfers] = useState<ExplorerTransfer[]>([])
  const [hiddenExplorerHistory, setHiddenExplorerHistory] = useState<Set<string>>(new Set())
  const [explorerTransferError, setExplorerTransferError] = useState(false)
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
  const [moving, setMoving] = useState(false)
  const choiceProps = useChoiceActivation()
  const [usage, setUsage] = useState<Record<string, StorageUsage>>({})
  const [preview, setPreview] = useState<{ entry: Entry; profileId: string; remotePath: string } | null>(null)
  const [share, setShare] = useState<{ entry: Entry; profileId: string; path: string; capabilities: ShareCapabilities } | null>(null)
  const [shareCaps, setShareCaps] = useState<{ id: string; value: ShareCapabilities } | null>(null)
  useEffect(() => {
    let disposed = false
    if (profile) void call<ShareCapabilities>('sharing_capabilities', { id: profile.id }).then(value => {
      if (!disposed) setShareCaps({ id: profile.id, value })
    }).catch(() => {})
    return () => { disposed = true }
  }, [profile])
  const loadSequence = useRef(0)
  const current = useRef({ profile, path, choose, blocked: false })
  const reloadRef = useRef<() => void>(() => {})
  const uploadRef = useRef<(paths: string[]) => void>(() => {})
  const notify = (message: string) => setToast(message)

  useEffect(() => {
    if (!desktop || !profiles.length) return
    let disposed = false
    let running = false
    const refresh = async () => {
      if (running || document.hidden) return
      running = true
      try {
        // Keep quota requests bounded; do not block browsing on unsupported providers.
        for (let offset = 0; offset < profiles.length && !disposed; offset += 3) {
          await Promise.all(profiles.slice(offset, offset + 3).map(async p => {
            const value = await call<StorageUsage>('storage_usage', { id: p.id }).catch(() => ({ used: null, total: null, free: null, message: 'Квота временно недоступна' }))
            if (!disposed) setUsage(current => ({ ...current, [p.id]: value }))
          }))
        }
      } finally { running = false }
    }
    void refresh()
    const onVisible = () => { if (!document.hidden) void refresh() }
    document.addEventListener('visibilitychange', onVisible)
    const timer = window.setInterval(onVisible, 60000)
    return () => { disposed = true; clearInterval(timer); document.removeEventListener('visibilitychange', onVisible) }
  }, [profiles])

  useEffect(() => { document.documentElement.dataset.theme = theme; localStorage.setItem('cloudloom-theme', theme) }, [theme])
  useEffect(() => { localStorage.setItem('cloudloom-view', view) }, [view])
  useEffect(() => { if (!toast) return; const timer = window.setTimeout(() => setToast(''), 4500); return () => clearTimeout(timer) }, [toast])
  useEffect(() => { current.current = { profile, path, choose, blocked: generalSettings || showTransfers || explorer || !!preview || !!share || !!prompt || moving } }, [profile, path, choose, preview, share, prompt, moving, explorer, showTransfers, generalSettings])
  useEffect(() => {
    if (!desktop) return
    const unlisten = listen('explorer-close-blocked', () => { setExplorer(true); setToast('Сначала отключите диски Проводника. Cloudloom продолжает отправлять ваши изменения.') })
    const blocked = listen<string>('cloudloom-close-blocked', ({ payload }) => setToast(payload))
    const installing = listen('winfsp-installing', () => { setExplorer(true); setToast('Завершите установку WinFsp в окне Windows, прежде чем закрывать Cloudloom.') })
    return () => { void unlisten.then(fn => fn()); void blocked.then(fn => fn()); void installing.then(fn => fn()) }
  }, [])
  useEffect(() => {
    if (!desktop) return
    call<Profile[]>('list_profiles').then(setProfiles).catch(e => setError(String(e)))
    call<Task[]>('list_tasks').then(setTasks).catch(e => setError(String(e)))
    const unlisten = listen<Task>('transfer', ({ payload }) => {
      setTasks(items => { const found = items.some(t => t.id === payload.id); return found ? items.map(t => t.id === payload.id ? payload : t) : [payload, ...items] })
      if (payload.status === 'complete' && payload.profileId === current.current.profile?.id) reloadRef.current()
    })
    const drop = getCurrentWebview().onDragDropEvent(event => {
      if (event.payload.type === 'over') setDragging(!current.current.blocked)
      else { setDragging(false); if (event.payload.type === 'drop' && !current.current.choose && !current.current.blocked) uploadRef.current(event.payload.paths) }
    })
    return () => { void unlisten.then(fn => fn()); void drop.then(fn => fn()) }
  }, [])

  useEffect(() => {
    if (!desktop) return
    let disposed = false
    let polling = false
    let completions = new Map<string, string | null>()
    const refresh = async () => {
      if (polling) return
      polling = true
      try {
        const rows = await call<ExplorerTransfer[]>('explorer_transfers')
        if (disposed || !Array.isArray(rows)) return
        const completed = rows.filter(row => row.status === 'complete')
        const key = (row: ExplorerTransfer) => JSON.stringify([row.profileId, row.path])
        const openProfile = current.current.profile?.id
        if (completed.some(row => row.profileId === openProfile && completions.get(key(row)) !== row.completedAt)) reloadRef.current()
        completions = new Map(completed.map(row => [key(row), row.completedAt]))
        setExplorerTransfers(rows)
        setExplorerTransferError(false)
      } catch {
        if (!disposed) setExplorerTransferError(true)
      } finally { polling = false }
    }
    void refresh()
    const timer = window.setInterval(() => void refresh(), 3000)
    return () => { disposed = true; window.clearInterval(timer) }
  }, [])

  useEffect(() => {
    if (!desktop) return
    let disposed = false
    const refresh = () => void call<number>('monthly_download_bytes').then(bytes => {
      if (!disposed && typeof bytes === 'number') setMonthlyDownloads(bytes)
    }).catch(() => {})
    refresh()
    const timer = window.setInterval(refresh, 60000)
    const unlisten = listen<Task>('transfer', ({ payload }) => {
      if (payload.status === 'complete' && payload.direction === 'download') refresh()
    })
    return () => { disposed = true; clearInterval(timer); void unlisten.then(fn => fn()) }
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
  const enter = (value: Profile) => { ++loadSequence.current; setProfile(value); setPath(''); setEntries([]); setSelected(new Set()); setQuery(''); setError(''); setChoose(false); setShowTransfers(false) }
  const navigate = (next: string) => { setPath(next); setSelected(new Set()); setQuery('') }
  const visible = useMemo(() => entries.filter(e => e.name.toLocaleLowerCase().includes(query.toLocaleLowerCase())).sort((a,b) => Number(b.isDir) - Number(a.isDir) || (sort === 'size' ? b.size - a.size : sort === 'date' ? b.modified.localeCompare(a.modified) : a.name.localeCompare(b.name))), [entries, query, sort])
  const picked = entries.filter(e => selected.has(e.path))
  const activeTasks = tasks.filter(t => ['active','queued'].includes(t.status))
  const transferCount = activeTasks.length + explorerTransfers.filter(t => ['active', 'queued', 'retrying'].includes(t.status)).length
  const syncByPath = useMemo(() => new Map(explorerTransfers.filter(t => t.profileId === profile?.id).map(t => [t.path, t])), [explorerTransfers, profile])
  const openTransfers = () => { setChoose(false); setShowTransfers(true) }
  const openFiles = () => { setShowTransfers(false); if (!profile) setChoose(true) }

  const updateOption = (key: string, value: string) => setOptions(o => ({ ...o, [key]: value }))
  const selectService = (id: string) => { setConnecting(id); setShowHelp(false); setFormName(service(id).name); setRoot(''); setOptions({}); setReply(null); setAdvanced(false); setConnectError('') }
  const openGuide = () => {
    if (!connecting) return
    if (desktop) void call('open_service_help', { provider: connecting }).catch(e => setConnectError(String(e)))
    else window.open(guideUrl(connecting), '_blank', 'noopener,noreferrer')
  }

  const receiveConnect = async (result: ConnectReply) => {
    setReply(result)
    if (result.profile) { setProfiles(await call<Profile[]>('list_profiles')); setConnecting(null); setOptions({}); setReply(null); notify('Профиль подключён'); enter(result.profile) }
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
  const closeConnect = () => { if (busy) return; if (reply?.id) void call('cancel_connect', { id: reply.id }); setConnecting(null); setOptions({}); setReply(null) }
  const disconnect = async (p: Profile) => {
    try { await call('disconnect_profile', { id: p.id }); setProfiles(items => items.filter(item => item.id !== p.id)); if (profile?.id === p.id) { setProfile(null); setEntries([]) }; notify('Профиль отключён. Данные в облаке сохранены.') } catch(e) { setError(String(e)) }
  }
  const toggle = (entry: Entry) => setSelected(items => { const next = new Set(items); if (next.has(entry.path)) next.delete(entry.path); else next.add(entry.path); return next })
  const openPreview = (entry: Entry) => { if (profile && !entry.isDir) setPreview({ entry, profileId: profile.id, remotePath: pathJoin(path, entry.path) }) }
  const { drag, rowProps, dropProps } = useFileInteractions({
    path, selected, setSelected, blocked: choose || !!preview || !!share || !!prompt || loading || moving || actionBusy,
    open: entry => entry.isDir ? navigate(pathJoin(path, entry.path)) : openPreview(entry),
    move: async (sources, destination) => {
      if (!profile || moving) return
      setMoving(true); setError('')
      try {
        const result = await call<{ moved: string[]; error: string | null }>('move_items', { id: profile.id, sources, destination })
        await reload()
        if (result.error) setError(`Перемещено: ${result.moved.length} из ${sources.length}. ${result.error}`)
        else notify(`Перемещено: ${result.moved.length}`)
      } catch (e) { await reload(); setError(String(e)) }
      finally { setMoving(false) }
    },
  })
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
  const retryTask = async (task: Task) => {
    try {
      await call<string>('retry_transfer', { taskId: task.id })
      setTasks(items => items.map(item => item.id === task.id ? { ...item, retryable: false } : item))
      setShowTransfers(true)
    } catch (e) { setError(String(e)) }
  }
  const dismissTransfer = async (task: Task) => {
    try {
      await call('dismiss_transfer', { id: task.id })
      setTasks(items => items.filter(item => item.id !== task.id))
    } catch (e) { setError(String(e)) }
  }
  const visibleExplorerTransfers = explorerTransfers.filter(task =>
    !task.completedAt || !hiddenExplorerHistory.has(explorerHistoryKey(task)),
  )
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

  return <div className={`app ${choose ? 'choosing' : ''}`} style={{ '--accent': theme === 'dark' ? liveUI.ui.accentDark : liveUI.ui.accentLight } as React.CSSProperties}>
    <div className="workspace" aria-hidden={choose || preview || share || explorer || generalSettings ? true : undefined} inert={choose || !!preview || !!share || explorer || moving || generalSettings || undefined}>
      <aside className="sidebar">
        <Brand />
        <p className="section-label">ПРОСТРАНСТВО</p>
        <button className={`nav ${showTransfers ? '' : 'active'}`} onClick={openFiles}><HardDrive size={18}/>Мои файлы</button>
        <button className={`nav ${showTransfers ? 'active' : ''}`} aria-current={showTransfers ? 'page' : undefined} onClick={openTransfers}><ArrowUpToLine size={18}/>Передачи{transferCount > 0 && <span className="count">{transferCount}</span>}</button>
        <p className="section-label profile-label">МОИ ПРОФИЛИ</p>
        {profiles.map(p => <div key={p.id} className="profile-nav-row"><button className={`profile-nav ${profile?.id === p.id ? 'selected' : ''}`} {...choiceProps(`profile:${p.id}`)} onDoubleClick={() => enter(p)} onKeyDown={e => { if (e.key === 'Enter') { e.preventDefault(); enter(p) } }}><ServiceIcon id={p.provider}/><span><strong>{p.name}</strong><small>{service(p.provider).name}</small><StorageMeter usage={usage[p.id]}/></span><i/></button><button className="profile-mount" title="Добавить в Проводник" aria-label={`В Проводник: ${p.name}`} onClick={() => mountProfile(p.id)}><HardDrive size={15}/></button></div>)}
        <button className="add-profile" onClick={() => setChoose(true)}><Plus size={16}/>Добавить профиль</button>
        <button className="add-profile" onClick={() => setExplorer(true)}><HardDrive size={16}/>Диски в Проводнике</button>
        <div className="sidebar-bottom"><div className="privacy"><Cloud size={17}/><span>Ваши файлы — в вашем облаке.<br/>Прямое соединение с хранилищем.</span></div><button className="theme-button" onClick={() => setTheme(t => t === 'dark' ? 'light' : 'dark')}>{theme === 'dark' ? <Sun size={17}/> : <Moon size={17}/>}<span>{theme === 'dark' ? 'Светлая тема' : 'Тёмная тема'}</span><small>{releaseName}</small></button></div>
      </aside>
      <main>
        <header className="topbar"><div className="breadcrumb"><Cloud size={17}/><span>{profile?.name ?? 'Ваше пространство'}</span><ChevronRight size={14}/><strong>{showTransfers ? 'Передачи' : path.split('/').pop() || 'Мои файлы'}</strong></div><div className="header-buttons"><span className="connection-state"><i/>Подключено</span>{desktop && <button className="round" aria-label="Общие настройки" title="Общие настройки" onClick={() => void openSettings()}><Settings2 size={18}/></button>}<button className="round" title="Выбрать профиль" aria-label="Выбрать профиль" onClick={() => setChoose(true)}><Menu size={19}/></button></div></header>
        {!showTransfers && <section className="files-content">
          {profile && <div className="active-storage"><HardDrive size={15}/><span>{profile.name} · <StorageMeter usage={usage[profile.id]}/></span><small>Квота аккаунта / диска · обновление раз в минуту</small></div>}
          <div className="files-heading"><div><p className="eyebrow">{profile ? service(profile.provider).name : 'CLOUDLOOM'}</p><h1>{path.split('/').pop() || 'Мои файлы'}<span>{entries.length}</span></h1><p className="description">{profile?.root ? `${profile.name} / ${profile.root}` : 'Всё нужное. В одном пространстве.'}</p></div><div className="file-actions"><button className="button subtle" onClick={() => { setText(''); setPrompt({kind:'folder'}) }} disabled={profile?.provider === 'google cloud storage'} title={profile?.provider === 'google cloud storage' ? 'В GCS папки создаются загрузкой объектов с префиксом' : undefined}><FolderPlus size={17}/>Новая папка</button><details className="upload-menu"><summary className="button primary"><Upload size={17}/>Загрузить<ChevronRight size={13}/></summary><div className="dropdown"><button onClick={() => void upload(false)}><File size={16}/>Файлы</button><button onClick={() => void upload(true)}><Folder size={16}/>Папку целиком</button></div></details></div></div>
          <div className="pathbar"><button disabled={!path} aria-label="На уровень выше" onClick={() => navigate(path.split('/').slice(0,-1).join('/'))}><ArrowLeft size={16}/></button><button {...dropProps('')} onDoubleClick={() => navigate('')} onKeyDown={e => { if (e.key === 'Enter') { e.preventDefault(); navigate('') } }}><HardDrive size={14}/>Корень</button>{path.split('/').filter(Boolean).map((part,index) => <span key={index}><ChevronRight size={13}/><button {...dropProps(path.split('/').slice(0,index+1).join('/'))} onDoubleClick={() => navigate(path.split('/').slice(0,index+1).join('/'))} onKeyDown={e => { if (e.key === 'Enter') { e.preventDefault(); navigate(path.split('/').slice(0,index+1).join('/')) } }}>{part}</button></span>)}</div>
          <div className="toolbar"><label className="search"><Search size={17}/><input placeholder="Поиск в этой папке" value={query} onChange={e => setQuery(e.target.value)}/><kbd>⌕</kbd></label><select aria-label="Сортировка" value={sort} onChange={e => setSort(e.target.value)}><option value="name">По имени</option><option value="date">По дате</option><option value="size">По размеру</option></select><button className="icon-button" title="Обновить" onClick={() => void reload()} disabled={loading}><RefreshCw size={17} className={loading ? 'spin' : ''}/></button><div className="view-buttons"><button aria-label="Список" className={view === 'list' ? 'active' : ''} onClick={() => setView('list')}><LayoutList size={17}/></button><button aria-label="Сетка" className={view === 'grid' ? 'active' : ''} onClick={() => setView('grid')}><Grid2X2 size={17}/></button></div></div>
          <p className="interaction-hint">Один клик — выделить · Двойной клик / Enter — открыть · Перетащите на папку или путь сверху, чтобы переместить</p>
          {moving && <div className="move-status" role="status">Перемещаем выбранное… Дождитесь завершения.</div>}
          <div className="selection" style={{ visibility: selected.size ? 'visible' : 'hidden' }}><span>Выбрано: {selected.size}</span><button disabled={moving} onClick={() => void download(picked)}><Download size={15}/>Скачать</button><button disabled={moving} className="danger-text" onClick={() => setPrompt({kind:'delete'})}><Trash2 size={15}/>Удалить</button><button aria-label="Снять выделение" onClick={() => setSelected(new Set())}><X size={15}/></button></div>
          {error && <div className="error" role="alert">{error}<button aria-label="Закрыть ошибку" onClick={() => setError('')}><X size={15}/></button></div>}
          {loading ? <div className="empty"><LoaderCircle className="spin" size={28}/><h3>Открываем ваши файлы</h3><p>Получаем содержимое хранилища…</p></div> : visible.length ? <div className={view === 'grid' ? 'file-grid' : 'file-list'}>
            {view === 'list' && <div className="list-head"><label><input aria-label="Выбрать всё" type="checkbox" checked={visible.length > 0 && visible.every(e => selected.has(e.path))} onChange={e => setSelected(e.target.checked ? new Set(visible.map(item => item.path)) : new Set())}/></label><span>Название</span><span>Изменён</span><span>Размер</span><span/></div>}
            {visible.map(entry => { const Icon = iconFor(entry)
              const sync = !entry.isDir && !explorerTransferError ? syncByPath.get(pathJoin(path, entry.path)) : undefined
              return <div {...rowProps(entry)} key={entry.path} onContextMenu={e => { e.preventDefault(); setSelected(new Set([entry.path])); const menu = e.currentTarget.querySelector('details'); if (menu) menu.open = true }}>
              <label className="file-check"><input type="checkbox" aria-label={`Выбрать ${entry.name}`} checked={selected.has(entry.path)} onChange={() => toggle(entry)}/></label><button className="entry-name"><span className={`file-symbol ${entry.isDir ? 'folder' : Icon === Film ? 'video' : Icon === FileAudio ? 'audio' : ''}`}><Icon size={view === 'grid' ? 32 : 21}/></span><strong>{entry.name}</strong>{sync && (sync.status !== 'complete' || sync.total === entry.size) && <SyncBadge status={sync.status}/>}</button><span className="file-date">{entry.modified && !entry.modified.startsWith('0001') ? new Date(entry.modified).toLocaleDateString('ru', { day:'numeric', month:'short', year:'numeric' }) : '—'}</span><span className="file-size">{entry.isDir ? 'Папка' : formatBytes(entry.size)}</span><details className="entry-menu"><summary aria-label={`Действия: ${entry.name}`}><MoreHorizontal size={18}/></summary><div className="dropdown">{!entry.isDir && <button onClick={() => openPreview(entry)}><FileText size={15}/>Открыть</button>}{profile && shareCaps?.id === profile.id && shareCaps.value.supported && <button onClick={() => setShare({ entry, profileId: profile.id, path: pathJoin(path, entry.path), capabilities: shareCaps.value })}><ExternalLink size={15}/>Получить ссылку</button>}<button onClick={() => void download([entry])}><Download size={15}/>Скачать{entry.isDir ? ' папку' : ''}</button><button onClick={() => { setText(entry.name); setPrompt({kind:'rename',entry}) }}><Settings2 size={15}/>Переименовать</button><button className="danger-text" onClick={() => setPrompt({kind:'delete',entry})}><Trash2 size={15}/>Удалить</button></div></details>
            </div> })}
          </div> : <div className="empty"><div className="empty-icon"><Folder size={34}/></div><h3>{query ? 'Ничего не найдено' : 'Здесь пока пусто'}</h3><p>{query ? 'Попробуйте другой поисковый запрос.' : 'Загрузите файлы или перетащите сюда папку.'}</p>{!query && <button className="button subtle" onClick={() => void upload(false)}><Plus size={16}/>Добавить файлы</button>}</div>}
          <footer className="file-footer"><span>{visible.length} элементов{selected.size ? ` · Выбрано ${selected.size}` : ''}</span><span><Cloud size={13}/>Обновлено с сервиса</span></footer>
        </section>}
        {showTransfers && <section className="files-content transfer-page" aria-label="Передачи">
          <div className="files-heading"><div><h1>Передачи</h1><p className="description">Загрузки и скачивания в Cloudloom и отправка файлов с дисков Проводника.</p></div></div>
          {error && <div className="error" role="alert">{error}<button aria-label="Закрыть ошибку" onClick={() => setError('')}><X size={15}/></button></div>}
          <section className="transfers">
            <header><span><ArrowUpToLine size={17}/>Из приложения</span></header>
            {tasks.length ? <div className="transfer-items">{tasks.map(task => {
              const running = task.status === 'active' || task.status === 'queued'
              return <div className={`transfer ${task.status}`} key={task.id}>
                <div className="transfer-direction">{task.direction === 'upload' ? <ArrowUpToLine size={18}/> : <ArrowDownToLine size={18}/>}</div>
                <div className="transfer-info">
                  <strong>{task.name}</strong>
                  <small>{statusText[task.status]}{task.total > 0 && ` · ${formatBytes(task.bytes)} / ${formatBytes(task.total)}`}{task.speed > 0 && ` · ${formatBytes(task.speed)}/с`}</small>
                  {task.status === 'active' && <progress max={task.total || 1} value={task.bytes}/>}
                  {task.error && <p>{task.error}</p>}
                </div>
                {running ? <button title="Отменить передачу" aria-label={`Отменить ${task.name}`} onClick={() => void call('cancel_transfer',{id:task.id}).catch(e => setError(String(e)))}><X size={15}/></button> : <>
                  {task.retryable && <button title="Повторить передачу" aria-label={`Повторить ${task.name}`} onClick={() => void retryTask(task)}><RefreshCw size={15}/></button>}
                  {task.status === 'complete' && <Check size={17}/>}
                  <button title="Убрать из истории" aria-label={`Убрать из истории: ${task.name}`} onClick={() => void dismissTransfer(task)}><X size={15}/></button>
                </>}
              </div>
            })}</div> : <div className="no-transfers">В приложении пока нет передач.</div>}
          </section>
          <section className="transfers">
            <header><span><HardDrive size={17}/>Из Проводника</span></header>
            {explorerTransferError ? <div className="no-transfers" role="status">Не удалось обновить очередь. Cloudloom повторит проверку.</div> :
              visibleExplorerTransfers.length ? <div className="transfer-items">{visibleExplorerTransfers.map(task =>
                <div className={`transfer ${task.status}`} key={JSON.stringify([task.profileId, task.path])}>
                  <div className="transfer-direction"><SyncBadge status={task.status}/></div>
                  <div className="transfer-info">
                    <strong title={task.path}>{task.letter}: / {task.path}</strong>
                    <small>
                      {task.status === 'active' && task.total > 0 && task.bytes === task.total ? 'Ожидает подтверждения сервера' : explorerStatus[task.status]}
                      {task.status === 'active' ? ` · ${formatBytes(task.bytes)} / ${formatBytes(task.total)}` : ` · ${formatBytes(task.total)}`}
                    </small>
                    {task.status === 'active' && <progress aria-label="Передача из Проводника" max={task.total || 1} value={task.bytes}/>}
                  </div>
                  {task.completedAt && <button
                    title="Убрать из истории"
                    aria-label={`Убрать из истории: ${task.path}`}
                    onClick={() => setHiddenExplorerHistory(history => new Set(history).add(explorerHistoryKey(task)))}
                  ><X size={15}/></button>}
                </div>
              )}</div> : <div className="no-transfers">На подключённых дисках нет передач.</div>}
          </section>
        </section>}
      </main>
    </div>

    {choose && <div className="profile-overlay" inert={explorer || generalSettings || undefined} aria-hidden={explorer || generalSettings || undefined}><div className="ambient ambient-one"/><div className="ambient ambient-two"/><div className="selector-top"><Brand/><div className="header-buttons">{desktop && <button className="round" aria-label="Общие настройки" title="Общие настройки" onClick={() => void openSettings()}><Settings2 size={18}/></button>}<button className="round" aria-label="Переключить тему" onClick={() => setTheme(t => t === 'dark' ? 'light' : 'dark')}>{theme === 'dark' ? <Sun size={18}/> : <Moon size={18}/>}</button></div></div><div className="welcome-title"><span className="welcome-tag"><i/>{liveUI.ui.tagline}</span><h1>{liveUI.ui.welcomeTitle}<br/><em>{liveUI.ui.welcomeEmphasis}</em></h1><p>{liveUI.ui.subtitle}</p><button className="button subtle explorer-entry" onClick={() => setExplorer(true)}><HardDrive size={16}/>Диски в Проводнике</button><button className="button subtle explorer-entry" onClick={openTransfers}><ArrowUpToLine size={16}/>Передачи{transferCount > 0 && ` · ${transferCount}`}</button></div>
      <div className="profile-picker" role="dialog" aria-label="Выбор профиля">
        <section className="saved-profiles"><div className="picker-heading"><Users size={17}/><h2>Мои профили</h2><span>{profiles.length}</span></div><p className="picker-description">Продолжите с нужным аккаунтом</p><div className="profile-cards">{profiles.length ? profiles.map(p => <div className="saved-profile" key={p.id}><button className="profile-open" {...choiceProps(`profile:${p.id}`)} onDoubleClick={() => enter(p)} onKeyDown={e => { if (e.key === 'Enter') { e.preventDefault(); enter(p) } }}><ServiceIcon id={p.provider}/><span><strong>{p.name}</strong><small>{service(p.provider).name}{p.root ? ` · ${p.root}` : ''}</small></span><ChevronRight size={17}/></button><button className="profile-mount" title="Добавить в Проводник" aria-label={`В Проводник: ${p.name}`} onClick={() => mountProfile(p.id)}><HardDrive size={15}/></button><button className="disconnect" title="Отключить профиль" aria-label={`Отключить ${p.name}`} onClick={() => void disconnect(p)}><LogOut size={13}/></button></div>) : <div className="no-profiles"><div><Cloudy size={31}/></div><h3>Здесь будут ваши профили</h3><p>Подключите сервис справа.<br/>В следующий раз аккаунт будет ждать вас здесь.</p></div>}</div><button className="new-profile" onClick={() => { setServiceQuery(''); document.getElementById('service-search')?.focus() }}><Plus size={19}/><span>Добавить профиль</span></button><div className="picker-security"><span className="lock-dot"/>OAuth-вход или ключи вашего сервиса.<br/>Учётные данные зашифрованы на компьютере.</div></section>
        <section className="available-services" id="services"><div className="picker-heading"><Cloud size={17}/><h2>Подключить хранилище</h2><span>{providers.length}</span></div><label className="service-search"><Search size={14}/><input id="service-search" aria-label="Найти сервис" placeholder="Найти сервис…" value={serviceQuery} onChange={e => setServiceQuery(e.target.value)}/>{serviceQuery && <button aria-label="Сбросить поиск сервисов" onClick={() => setServiceQuery('')}><X size={13}/></button>}</label><div className="services">{providers.filter(p => `${p.name} ${p.description}`.toLocaleLowerCase().includes(serviceQuery.toLocaleLowerCase().trim())).map(p => <button className="service-option" key={p.id} {...choiceProps(`service:${p.id}`)} onDoubleClick={() => selectService(p.id)} onKeyDown={e => { if (e.key === 'Enter') { e.preventDefault(); selectService(p.id) } }}><ServiceIcon id={p.id}/><span><strong>{p.name}</strong><small>{p.description}</small></span><Plus size={17}/></button>)}{!providers.some(p => `${p.name} ${p.description}`.toLocaleLowerCase().includes(serviceQuery.toLocaleLowerCase().trim())) && <p className="service-empty" role="status">Сервис не найден. Попробуйте другое название.</p>}</div></section>
      </div><div className="welcome-footer"><span>Cloudloom {releaseName}</span><button className="live-ui-check" onClick={() => void liveUI.check()} disabled={liveUI.checking} title={`${liveUI.status} · каталог ${liveUI.revision}`}><RefreshCw size={11} className={liveUI.checking ? 'spin' : ''}/>{liveUI.checking ? 'Проверяем интерфейс…' : 'Проверить интерфейс'}</button><span>Файлы не проходят через наши серверы</span></div>{monthlyDownloads !== null && <small
        className="monthly-downloads"
        title="Успешные скачивания через Cloudloom на этом компьютере. Счётчик начинается заново в новом месяце."
      >За месяц скачано · {new Intl.NumberFormat('ru-RU', { maximumFractionDigits: 2 }).format(monthlyDownloads / 1e9)} ГБ</small>}{!desktop && <div className="browser-note">Предпросмотр интерфейса. Для входа и работы с файлами запустите приложение для macOS или Windows.</div>}{error && <div className="selector-error error">{error}<button onClick={() => setError('')}><X size={15}/></button></div>}
    </div>}

    {generalSettings && <div className="modal-backdrop"><section className="modal" role="dialog" aria-modal="true" aria-labelledby="general-settings-title" onKeyDown={e => { if (e.key === 'Escape') setGeneralSettings(false) }}>
      <button className="modal-close" aria-label="Закрыть настройки" onClick={() => setGeneralSettings(false)}><X size={18}/></button>
      <h2 id="general-settings-title">Общие настройки</h2>
      <label>При закрытии окна
        <select autoFocus value={closeMode} disabled={savingSettings} onChange={e => void changeCloseMode(e.target.value as CloseMode)}>
          <option value="keepOpen">Оставить окно открытым, пока диски подключены</option>
          <option value="tray">Свернуть в трей и оставить диски подключёнными</option>
          <option value="disconnect">Отключить диски и выйти</option>
        </select>
      </label>
      {closeMode === 'tray' && <p className="setup-note">Диски и передачи продолжат работать. Открыть окно или выйти можно через значок Cloudloom.</p>}
      {closeMode === 'disconnect' && <p className="setup-note">Если есть незавершённые передачи, окно останется открытым.</p>}
      {settingsError && <p className="error" role="alert">{settingsError}</p>}
    </section></div>}

    {connecting && <div className="modal-backdrop"><div className="modal connect-modal" role="dialog" aria-modal="true" aria-labelledby="connect-title"><button className="modal-close" aria-label="Закрыть подключение" onClick={closeConnect} disabled={busy}><X size={18}/></button><ServiceIcon id={connecting}/><div className="connect-heading"><h2 id="connect-title">{service(connecting).name}</h2><button className="help-toggle" type="button" aria-label={`Помощь с подключением ${service(connecting).name}`} aria-expanded={showHelp} aria-controls="service-help" title="Как получить ключи и войти" onClick={() => setShowHelp(v => !v)}><CircleHelp size={16}/><span>Помощь</span></button></div><p className="modal-subtitle">{busy || reply?.question ? 'Завершите настройку подключения' : 'Добавьте свой профиль в Cloudloom'}</p>
      {showHelp && <section id="service-help" className="service-help" aria-label={`Инструкция для ${service(connecting).name}`}><h3>Как подключить {service(connecting).name}</h3><p>{guides[connecting].intro}</p><ol>{guides[connecting].steps.map(step => <li key={step}>{step}</li>)}</ol>{guides[connecting].note && <p className="help-note">{guides[connecting].note}</p>}<button className="help-link" type="button" onClick={openGuide}>Официальная инструкция<ExternalLink size={13}/></button><small>Откроется в браузере. Никому не передавайте ключи и токены.</small></section>}
      {busy ? <div className="auth-wait"><LoaderCircle size={31} className="spin"/><h3>Подключаем профиль…</h3><p>{service(connecting).oauth ? 'Официальная страница входа откроется в системном браузере. Выберите аккаунт и разрешите доступ. Окно Cloudloom обновится после входа.' : 'Проверяем учётные данные и доступ к хранилищу. Это может занять некоторое время.'}</p></div> : <form onSubmit={e => { e.preventDefault(); void connect() }}>
        {reply?.question ? <div className="question"><p>{reply.error}</p><label>{reply.question.help}{reply.question.examples?.length ? <select value={answer} onChange={e => setAnswer(e.target.value)}>{!reply.question.exclusive && <option value={answer}>{answer || 'По умолчанию'}</option>}{reply.question.examples.map((example,index) => <option key={index} value={example.Value}>{example.Help || example.Value}</option>)}</select> : typeof reply.question.default === 'boolean' ? <select value={answer} onChange={e => setAnswer(e.target.value)}><option value="true">Да</option><option value="false">Нет</option></select> : <input autoFocus type={reply.question.password ? 'password' : 'text'} value={answer} onChange={e => setAnswer(e.target.value)}/>}</label></div> : <>
          <label>Название профиля<input required autoFocus value={formName} onChange={e => setFormName(e.target.value)} placeholder="Например: Рабочий аккаунт"/></label>
          {connecting === 'alias' && <label>Локальная папка<div className="input-with-button"><input required value={root} onChange={e => setRoot(e.target.value)} placeholder="Выберите папку на компьютере"/><button type="button" onClick={() => void open({directory:true}).then(p => typeof p === 'string' && setRoot(p)).catch(e => setConnectError(String(e)))}>Выбрать</button></div></label>}
          <p className="setup-summary">{connectionSummary(connecting)}</p>
          {guides[connecting].fields?.filter(field => !field.advanced || advanced).map(field => <label key={field.key}>{field.label}<input required={field.required} type={field.secret ? 'password' : 'text'} autoComplete={field.key === '2fa' ? 'one-time-code' : 'off'} value={options[field.key] ?? ''} placeholder={field.placeholder} onChange={e => updateOption(field.key,e.target.value)}/>{['key_file','known_hosts_file'].includes(field.key) && <button className="help-link" type="button" onClick={() => void open({ multiple:false, title:field.label }).then(p => { if (typeof p === 'string') updateOption(field.key,p) }).catch(e => setConnectError(String(e)))}>Выбрать файл…</button>}</label>)}
          {!service(connecting).oauth && guides[connecting].fields?.some(field => field.advanced) && <button className="advanced-toggle" type="button" aria-expanded={advanced} onClick={() => setAdvanced(v => !v)}><Settings2 size={14}/>{advanced ? 'Скрыть дополнительные настройки' : 'Дополнительные настройки'}<ChevronRight size={13}/></button>}
          {['b2','azureblob','storj','sftp'].includes(connecting) && <label>{connecting === 'b2' || connecting === 'storj' ? 'Бакет (необязательно)' : connecting === 'azureblob' ? 'Контейнер (необязательно)' : 'Папка относительно домашней (необязательно)'}<input value={root} onChange={e => setRoot(e.target.value)} placeholder="Пусто — корень хранилища"/></label>}
          {connecting === 'seafile' && <label>Двухфакторная авторизация<select value={options['2fa'] ?? 'false'} onChange={e => updateOption('2fa',e.target.value)}><option value="false">Не включена</option><option value="true">Включена — запросить код</option></select></label>}
          {connecting === 'google cloud storage' && <><label>Бакет (необязательно)<input value={root} onChange={e => setRoot(e.target.value)} placeholder="my-studio-bucket"/></label><label>Номер проекта<input value={options.project_number ?? ''} onChange={e => updateOption('project_number',e.target.value)} placeholder="Нужен для списка бакетов"/></label></>}
          {connecting === 'webdav' && <><label>Адрес WebDAV<input required type="url" value={options.url ?? ''} onChange={e => updateOption('url',e.target.value)} placeholder="https://cloud.example.com/remote.php/dav/files/user/"/></label><label>Пользователь<input value={options.user ?? ''} onChange={e => updateOption('user',e.target.value)}/></label><label>Пароль приложения<input type="password" value={options.pass ?? ''} onChange={e => updateOption('pass',e.target.value)} autoComplete="off"/></label></>}
          {['wasabi','r2'].includes(connecting) && <label>Бакет<input required value={root} onChange={e => setRoot(e.target.value)} placeholder="Имя существующего бакета"/></label>}
          {connecting === 's3' && <><label>Бакет<input required value={root} onChange={e => setRoot(e.target.value)}/></label><label>Endpoint<input value={options.endpoint ?? ''} onChange={e => { updateOption('endpoint',e.target.value); updateOption('provider','Other') }} placeholder="Пусто для Amazon AWS"/></label><label>Регион<input value={options.region ?? ''} onChange={e => updateOption('region',e.target.value)} placeholder="eu-west-1"/></label><label>Access Key ID<input required value={options.access_key_id ?? ''} onChange={e => updateOption('access_key_id',e.target.value)}/></label><label>Secret Access Key<input required type="password" autoComplete="off" value={options.secret_access_key ?? ''} onChange={e => updateOption('secret_access_key',e.target.value)}/></label></>}
          {service(connecting).oauth && <><button className="advanced-toggle" type="button" onClick={() => setAdvanced(v => !v)}><Settings2 size={14}/>Настройки OAuth<ChevronRight size={13}/></button>{connecting === 'drive' && <p className="setup-note">Google переводит приложения на собственные OAuth Client ID. Если стандартный вход недоступен, укажите Client ID и Client Secret вашего Desktop OAuth приложения в настройках ниже.</p>}{advanced && <div className="advanced"><label>OAuth Client ID<input value={options.client_id ?? ''} onChange={e => updateOption('client_id',e.target.value)}/></label><label>OAuth Client Secret (если требуется)<input type="password" autoComplete="off" value={options.client_secret ?? ''} onChange={e => updateOption('client_secret',e.target.value)}/></label><p>Настройки и токены хранятся в зашифрованном виде на этом компьютере.</p></div>}</>}
        </>}
        {connectError && <div className="error" role="alert">{connectError}</div>}
        <button className="button primary connect-submit" type="submit">{reply?.question ? 'Продолжить' : service(connecting).oauth ? 'Войти через браузер' : 'Подключить профиль'}<ChevronRight size={16}/></button>
      </form>}
    </div></div>}
    {prompt && <div className="modal-backdrop"><form className="modal action-modal" role="dialog" aria-modal="true" onSubmit={e => { e.preventDefault(); void submitAction() }}><button type="button" className="modal-close" aria-label="Закрыть диалог" disabled={actionBusy} onClick={() => setPrompt(null)}><X size={18}/></button><h2>{prompt.kind === 'folder' ? 'Новая папка' : prompt.kind === 'rename' ? 'Переименовать' : 'Удалить выбранное?'}</h2>{prompt.kind === 'delete' ? <p className="modal-subtitle">{prompt.entry ? prompt.entry.name : `${picked.length} элементов`}. Папки удаляются вместе со всем содержимым. Восстановление зависит от сервиса.</p> : <label>Название<input autoFocus required value={text} onChange={e => setText(e.target.value)}/></label>}<div className="dialog-actions"><button type="button" className="button subtle" disabled={actionBusy} onClick={() => setPrompt(null)}>Отмена</button><button className={`button ${prompt.kind === 'delete' ? 'danger' : 'primary'}`} disabled={actionBusy || (prompt.kind !== 'delete' && !text.trim())}>{actionBusy && <LoaderCircle size={15} className="spin"/>}{prompt.kind === 'delete' ? 'Удалить' : 'Сохранить'}</button></div>{error && <p className="error">{error}</p>}</form></div>}
    {dragging && !choose && !preview && !share && !prompt && !moving && !explorer && <div className="drop-overlay"><Upload size={40}/><h2>Отпустите, чтобы загрузить</h2><p>Файлы и папки попадут в текущую папку</p></div>}
    {drag && <div className="drag-ghost" style={{ left: Math.min(drag.x + 16, window.innerWidth - 260), top: Math.min(drag.y + 16, window.innerHeight - 90) }}>Переместить: {drag.sources.length}<small>{drag.destination !== null ? `В папку: ${drag.destination || 'Корень'}` : 'Наведите на папку или путь сверху · Esc — отмена'}</small></div>}
    {preview && <FilePreview profileId={preview.profileId} remotePath={preview.remotePath} entry={preview.entry} onClose={() => setPreview(null)} onSaved={() => { void reload(); setProfiles(items => [...items]) }} onDownload={() => void download([preview.entry])}/>}
    {share && <ShareDialog profileId={share.profileId} path={share.path} entry={share.entry} capabilities={share.capabilities} onClose={() => setShare(null)}/>}
    <ExplorerDisks ref={explorerPanel} profiles={profiles} open={explorer} onClose={() => setExplorer(false)}/>
    {toast && <div className="toast" role="status"><Check size={16}/>{toast}</div>}
  </div>
}

function StorageMeter({ usage }: { usage?: StorageUsage }) {
  const percent = usagePercent(usage)
  return <span className="storage-meter" title={usage?.message ?? 'Объём аккаунта или диска, сообщённый сервисом'}><small>{usageLabel(usage)}</small>{percent !== null && <progress aria-label="Занято места" max={100} value={percent}/>}</span>
}
function Brand() { return <div className="brand"><span className="brand-logo"><Cloudy size={23}/></span><strong>cloudloom<span>®</span></strong></div> }
function ServiceIcon({id}:{id:string}) { const p = service(id); return <span className={`service-icon service-${id.replaceAll(' ','-')}`} style={{'--service':p.color} as React.CSSProperties}>{id === 'drive' ? <svg width="24" height="24" viewBox="0 0 24 24" aria-hidden="true"><path fill="#25a65a" d="M8 2h8L5 21 1 14z"/><path fill="#f6c344" d="M8 2h8l8 14h-8z"/><path fill="#4285f4" d="M5 21l4-7h15l-4 7z"/></svg> : id === 'onedrive' ? <Cloud size={25} fill="currentColor"/> : id === 'alias' ? <HardDrive size={22}/> : p.letters}</span> }
