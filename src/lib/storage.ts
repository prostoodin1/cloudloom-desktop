import type { StorageUsage } from '../api'
import { formatBytes } from './format'

export function usageLabel(usage?: StorageUsage) {
  if (!usage) return 'Проверяем объём…'
  if (usage.used !== null && usage.total !== null) return `${formatBytes(usage.used)} из ${formatBytes(usage.total)} занято`
  if (usage.used !== null) return `${formatBytes(usage.used)} занято · лимит неизвестен`
  if (usage.total !== null) return `Всего ${formatBytes(usage.total)} · занято неизвестно`
  if (usage.free !== null) return `${formatBytes(usage.free)} свободно · лимит неизвестен`
  return 'Объём не предоставлен'
}
export function usagePercent(usage?: StorageUsage) {
  return usage?.used !== null && usage?.used !== undefined && usage.total !== null && usage.total > 0
    ? Math.min(100, Math.max(0, usage.used / usage.total * 100)) : null
}
