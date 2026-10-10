import { useEffect, useRef, useState, type MouseEvent, type PointerEvent, type KeyboardEvent } from 'react'
import type { Entry } from './api'

export function canDrop(sources: string[], destination: string) {
  return sources.length > 0 && sources.every(source => source !== destination && !destination.startsWith(`${source}/`) && (source.split('/').slice(0, -1).join('/')) !== destination)
}

type Drag = { sources: string[]; x: number; y: number; destination: string | null }
export function useFileInteractions({ path, selected, setSelected, open, move, blocked }: {
  path: string; selected: Set<string>; setSelected: (value: Set<string>) => void
  open: (entry: Entry) => void; move: (sources: string[], destination: string) => Promise<void>; blocked: boolean
}) {
  const [drag, setDrag] = useState<Drag | null>(null)
  const cancel = useRef<(() => void) | null>(null)
  const suppressClick = useRef(0)
  useEffect(() => () => cancel.current?.(), [path, blocked])
  const join = (part: string) => [path, part].filter(Boolean).join('/')
  const rowProps = (entry: Entry) => ({
    'data-drop-path': entry.isDir ? join(entry.path) : undefined,
    className: `file-row ${selected.has(entry.path) ? 'selected' : ''} ${drag?.destination === join(entry.path) ? 'drop-target' : ''}`,
    onClick: (event: MouseEvent<HTMLDivElement>) => {
      if (Date.now() < suppressClick.current || (event.target as HTMLElement).closest('label,details')) return
      event.currentTarget.querySelector<HTMLButtonElement>('.entry-name')?.focus({ preventScroll: true })
      setSelected(event.ctrlKey || event.metaKey ? new Set(selected.has(entry.path) ? [...selected].filter(p => p !== entry.path) : [...selected, entry.path]) : new Set([entry.path]))
    },
    onDoubleClick: (event: MouseEvent<HTMLDivElement>) => {
      if (!blocked && Date.now() >= suppressClick.current && !(event.target as HTMLElement).closest('label,details')) open(entry)
    },
    onKeyDown: (event: KeyboardEvent<HTMLDivElement>) => {
      if (event.key === 'Enter' && (event.target as HTMLElement).closest('.entry-name')) { event.preventDefault(); if (!blocked) open(entry) }
    },
    onPointerDown: (event: PointerEvent<HTMLDivElement>) => {
      if (blocked || event.button !== 0 || event.ctrlKey || event.metaKey || (event.target as HTMLElement).closest('label,details')) return
      cancel.current?.()
      const node = event.currentTarget
      const sources = (selected.has(entry.path) ? [...selected] : [entry.path]).map(join)
      const origin = { x: event.clientX, y: event.clientY }
      let started = false
      let destination: string | null = null
      const update = (e: globalThis.PointerEvent) => {
        if (e.pointerId !== event.pointerId) return
        if (!started && Math.hypot(e.clientX - origin.x, e.clientY - origin.y) < 7) return
        if (!started) { started = true; node.setPointerCapture(event.pointerId); setSelected(new Set(sources.map(s => path ? s.slice(path.length + 1) : s))) }
        e.preventDefault()
        const target = document.elementFromPoint(e.clientX, e.clientY)?.closest<HTMLElement>('[data-drop-path]')
        const candidate = target?.dataset.dropPath
        destination = candidate !== undefined && canDrop(sources, candidate) ? candidate : null
        setDrag({ sources, x: e.clientX, y: e.clientY, destination })
      }
      const cleanup = () => {
        window.removeEventListener('pointermove', update)
        window.removeEventListener('pointerup', finish)
        window.removeEventListener('pointercancel', abort)
        window.removeEventListener('keydown', escape)
        window.removeEventListener('blur', abort)
        if (node.hasPointerCapture(event.pointerId)) node.releasePointerCapture(event.pointerId)
        if (started) suppressClick.current = Date.now() + 400
        setDrag(null); cancel.current = null
      }
      const finish = (e: globalThis.PointerEvent) => {
        if (e.pointerId !== event.pointerId) return
        const target = destination
        cleanup()
        if (started && target !== null) void move(sources, target)
      }
      const abort = () => cleanup()
      const escape = (e: globalThis.KeyboardEvent) => { if (e.key === 'Escape') cleanup() }
      cancel.current = cleanup
      window.addEventListener('pointermove', update, { passive: false })
      window.addEventListener('pointerup', finish)
      window.addEventListener('pointercancel', abort)
      window.addEventListener('keydown', escape)
      window.addEventListener('blur', abort)
    },
  })
  return { drag, rowProps, dropProps: (destination: string) => ({ 'data-drop-path': destination, className: drag?.destination === destination ? 'drop-target' : undefined }) }
}

export function useChoiceActivation() {
  const [choice, setChoice] = useState('')
  return (id: string) => ({
    'aria-pressed': choice === id,
    onClick: () => setChoice(id),
  })
}
