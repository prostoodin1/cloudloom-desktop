export function formatBytes(bytes: number | null) {
  if (bytes === null || bytes < 0 || !Number.isFinite(bytes)) return '—'
  if (bytes === 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const unit = Math.min(Math.floor(Math.log(bytes) / Math.log(1000)), units.length - 1)
  return `${(bytes / 1000 ** unit).toFixed(unit > 1 ? 1 : 0)} ${units[unit]}`
}
export function formatDate(value: string) { return new Intl.DateTimeFormat('en', { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' }).format(new Date(value)) }
