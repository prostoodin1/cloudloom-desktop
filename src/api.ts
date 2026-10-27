import { invoke, isTauri } from '@tauri-apps/api/core'
export interface Profile { id: string; name: string; provider: string; root: string; ready: boolean; transport?: string }
export interface Entry { name: string; path: string; size: number; isDir: boolean; modified: string; mime: string }
export interface StorageUsage { used: number | null; total: number | null; free: number | null; message: string | null }
export interface PreviewData { token: string; kind: 'image' | 'video' | 'audio' | 'text'; localPath: string | null; streamUrl?: string | null; text: string | null }
export interface ShareCapabilities { supported: boolean; delivery: boolean }
export interface ShareData { url: string; id: string | null; expiresAt: number | null; canRevoke: boolean }
export interface Task { id: string; profileId: string; name: string; direction: string; status: string; bytes: number; total: number; speed: number; error?: string }
export interface Question { name: string; help: string; default: unknown; password: boolean; exclusive: boolean; examples: { Value: string; Help: string }[] | null }
export interface ConnectReply { profile?: Profile; id?: string; question?: Question; error?: string }
export const desktop = isTauri()
export function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!desktop) return Promise.reject(new Error('Для подключения и работы с файлами откройте приложение Cloudloom для macOS или Windows.'))
  return invoke<T>(command, args)
}
export const providers = [
  { id: 'drive', name: 'Google Drive', description: 'Личный диск и общие папки', color: '#29a56c', letters: 'G', oauth: true },
  { id: 'onedrive', name: 'OneDrive', description: 'Microsoft 365 и личные файлы', color: '#238ae6', letters: 'O', oauth: true },
  { id: 'dropbox', name: 'Dropbox', description: 'Файлы и совместная работа', color: '#3875f6', letters: 'D', oauth: true },
  { id: 'google cloud storage', name: 'Google Cloud Storage', description: 'Бакеты и большие медиафайлы', color: '#e6a331', letters: 'GC', oauth: true },
  { id: 'box', name: 'Box', description: 'Облачное пространство команды', color: '#3970db', letters: 'B', oauth: true },
  { id: 'pcloud', name: 'pCloud', description: 'Личный облачный архив', color: '#16a4a5', letters: 'p', oauth: true },
  { id: 'yandex', name: 'Яндекс Диск', description: 'Ваши файлы на Диске', color: '#efb917', letters: 'Я', oauth: true },
  { id: 'webdav', name: 'WebDAV / Nextcloud', description: 'Свой сервер или облако', color: '#7486a7', letters: 'W', oauth: false },
  { id: 's3', name: 'Amazon S3 / S3 compatible', description: 'AWS, Wasabi, MinIO и другие', color: '#ba7b39', letters: 'S3', oauth: false },
  { id: 'alias', name: 'Локальная папка', description: 'Файлы на этом компьютере', color: '#8c83c5', letters: 'PC', oauth: false },
  { id: 'mega', name: 'MEGA', description: 'Зашифрованное личное облако', color: '#db4a4a', letters: 'M', oauth: false },
  { id: 'koofr', name: 'Koofr', description: 'Европейское файловое хранилище', color: '#73ad45', letters: 'K', oauth: false },
  { id: 'hidrive', name: 'HiDrive', description: 'Облако STRATO / IONOS', color: '#438bc7', letters: 'H', oauth: true },
  { id: 'b2', name: 'Backblaze B2', description: 'Объектное хранилище и архивы', color: '#dc5050', letters: 'B2', oauth: false },
  { id: 'azureblob', name: 'Azure Blob Storage', description: 'Контейнеры Microsoft Azure', color: '#3299d8', letters: 'Az', oauth: false },
  { id: 'storj', name: 'Storj', description: 'Распределённое хранилище', color: '#6385e0', letters: 'St', oauth: false },
  { id: 'seafile', name: 'Seafile', description: 'Библиотеки на вашем сервере', color: '#e6a64b', letters: 'Sf', oauth: false },
  { id: 'sftp', name: 'SFTP', description: 'Сервер по защищённому SSH', color: '#aa8dd1', letters: 'SSH', oauth: false },
  { id: 'delivery', name: 'Delivery', description: 'Вход по адресу сайта, логину и паролю', color: '#c47361', letters: 'D', oauth: false },
  { id: 'nextcloud', name: 'Nextcloud', description: 'Личное или корпоративное облако', color: '#0082c9', letters: 'N', oauth: false },
  { id: 'owncloud', name: 'ownCloud', description: 'Файлы на своём сервере', color: '#345b91', letters: 'OC', oauth: false },
  { id: 'protondrive', name: 'Proton Drive', description: 'Зашифрованное облако · адаптер beta', color: '#8a6be8', letters: 'P', oauth: false },
  { id: 'opendrive', name: 'OpenDrive', description: 'Облачные файлы и резервные копии', color: '#4598c9', letters: 'OD', oauth: false },
  { id: 'jottacloud', name: 'Jottacloud', description: 'Подключение по Personal login token', color: '#54a887', letters: 'J', oauth: false },
  { id: 'pikpak', name: 'PikPak', description: 'Медиафайлы и личное хранилище', color: '#5579de', letters: 'PK', oauth: false },
  { id: 'wasabi', name: 'Wasabi', description: 'S3-хранилище по ключам доступа', color: '#73a644', letters: 'W', oauth: false },
  { id: 'r2', name: 'Cloudflare R2', description: 'Бакеты Cloudflare и S3 API', color: '#e18a3b', letters: 'R2', oauth: false },
]
export const service = (id: string) => providers.find(p => p.id === id) ?? providers[0]
