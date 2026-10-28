import { createHash } from 'node:crypto'
import { existsSync, readFileSync, mkdirSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { spawnSync } from 'node:child_process'

export const winFspHash = '073a70e00f77423e34bed98b86e600def93393ba5822204fac57a29324db9f7a'
export function verifyWinFsp(bytes) {
  if (createHash('sha256').update(bytes).digest('hex') !== winFspHash) throw new Error('WinFsp installer checksum mismatch')
}
if (process.platform === 'win32') {
  const root = dirname(dirname(fileURLToPath(import.meta.url)))
  const folder = join(root, 'src-tauri/resources/winfsp')
  const file = join(folder, 'winfsp-2.1.25156.msi')
  mkdirSync(folder, { recursive: true })
  if (!existsSync(file)) {
    const response = await fetch('https://github.com/winfsp/winfsp/releases/download/v2.1/winfsp-2.1.25156.msi', { signal: AbortSignal.timeout(90000) })
    if (!response.ok) throw new Error(`WinFsp download failed: ${response.status}`)
    const bytes = Buffer.from(await response.arrayBuffer())
    verifyWinFsp(bytes)
    writeFileSync(file, bytes)
  }
  verifyWinFsp(readFileSync(file))
  const nativeModules = join(process.env.SystemRoot || 'C:/Windows', 'System32/WindowsPowerShell/v1.0/Modules')
  const result = spawnSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', "if ((Get-AuthenticodeSignature -LiteralPath $env:CLOUDLOOM_WINFSP_MSI).Status -ne 'Valid') { throw 'WinFsp Authenticode verification failed' }"], { env: { ...process.env, PSModulePath: nativeModules, CLOUDLOOM_WINFSP_MSI: file }, stdio: 'inherit', windowsHide: true })
  if (result.error || result.status !== 0) throw new Error('WinFsp Authenticode verification failed')
  console.log('Verified bundled WinFsp 2.1.25156 (SHA256 and Authenticode).')
}
