import { createHash } from 'node:crypto'
import { existsSync, mkdtempSync, writeFileSync, mkdirSync, copyFileSync, chmodSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { spawnSync } from 'node:child_process'

const root = dirname(dirname(fileURLToPath(import.meta.url)))
const licenseFolder = join(root, 'src-tauri/resources')
mkdirSync(licenseFolder, { recursive: true })
copyFileSync(join(root, 'LICENSE'), join(licenseFolder, 'CLOUDLOOM-LICENSE.txt'))
if (process.platform === 'win32') {
  await import('./prepare-winfsp.mjs')
  const result = spawnSync('powershell', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', join(root, 'scripts/prepare-engine.ps1')], { stdio: 'inherit' })
  if (result.error) throw result.error
  process.exit(result.status ?? 1)
}
if (process.platform !== 'darwin') throw new Error('Native Cloudloom builds currently support macOS and Windows.')
const distributions = {
  x64: { name: 'osx-amd64', sha256: '574e1f31cf1c464e5fb0325cb66fcbdeaf1efa1802bd9b17f13c256fd74b6561', arch: 'amd64' },
  arm64: { name: 'osx-arm64', sha256: '610ab7d48615135dfc60b763858b11a426a0d3d1160c1bb76fb0ae6011b02edd', arch: 'arm64' },
}
const distribution = distributions[process.arch]
if (!distribution) throw new Error(`Unsupported macOS architecture: ${process.arch}`)
const resources = join(root, 'src-tauri/resources')
const engine = join(resources, 'rclone')
function verifyVersion() {
  const result = spawnSync(engine, ['version'], { encoding: 'utf8' })
  if (result.status !== 0 || !result.stdout.startsWith('rclone v1.75.2\n') || !result.stdout.includes(`os/arch: ${distribution.arch}`)) {
    throw new Error('Unexpected rclone version or architecture. Remove only src-tauri/resources/rclone and prepare again.')
  }
}
if (existsSync(engine)) {
  verifyVersion()
  process.exit(0)
}
const temporary = mkdtempSync(join(tmpdir(), 'cloudloom-engine-'))
const archiveName = `rclone-v1.75.2-${distribution.name}`
const response = await fetch(`https://downloads.rclone.org/v1.75.2/${archiveName}.zip`)
if (!response.ok) throw new Error(`Engine download failed: HTTP ${response.status}`)
const archive = Buffer.from(await response.arrayBuffer())
if (createHash('sha256').update(archive).digest('hex') !== distribution.sha256) throw new Error('rclone archive checksum mismatch')
const archivePath = join(temporary, 'rclone.zip')
writeFileSync(archivePath, archive)
const extraction = spawnSync('/usr/bin/unzip', ['-q', archivePath, '-d', temporary], { stdio: 'inherit' })
if (extraction.status !== 0) throw new Error('Cannot extract the verified rclone archive')
mkdirSync(resources, { recursive: true })
copyFileSync(join(temporary, archiveName, 'rclone'), engine)
chmodSync(engine, 0o755)
verifyVersion()
console.log(`Verified rclone v1.75.2 (${distribution.name}) is ready.`)
