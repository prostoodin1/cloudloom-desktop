import { readFileSync, writeFileSync, statSync } from 'node:fs'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

export function createManifest({ directory, repository, tag, version, name, date = new Date().toISOString() }) {
  if (!/^[A-Za-z0-9][\w.-]*\/[A-Za-z0-9][\w.-]*$/.test(repository) || !/^cloudloom-[a-z]+$/.test(tag) || !/^\d+\.\d+\.\d+$/.test(version) || !/^[A-Za-z]+$/.test(name)) throw Error('Invalid release metadata')
  const prefix = `Cloudloom_${name}`
  const files = { 'windows-x86_64': `${prefix}_Windows_x64_Setup.exe`, 'darwin-x86_64': `${prefix}_macOS_Intel.app.tar.gz`, 'darwin-aarch64': `${prefix}_macOS_AppleSilicon.app.tar.gz` }
  const platforms = Object.fromEntries(Object.entries(files).map(([platform, file]) => {
    if (!statSync(join(directory, file)).isFile() || statSync(join(directory, file)).size === 0) throw Error(`Missing package ${file}`)
    const signature = readFileSync(join(directory, `${file}.sig`), 'utf8').trim()
    if (!/^[A-Za-z0-9+/=]+$/.test(signature) || signature.length < 100) throw Error(`Invalid signature ${file}`)
    return [platform, { signature, url: `https://github.com/${repository}/releases/download/${tag}/${file}` }]
  }))
  return { version, notes: `Cloudloom ${name}`, pub_date: date, platforms }
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const config = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8'))
  const manifest = createManifest({ directory: 'release-assets', repository: process.env.GITHUB_REPOSITORY, tag: process.env.RELEASE_TAG, version: config.version, name: config.app.windows[0].title.replace('Cloudloom ', '') })
  writeFileSync('release-assets/latest.json', JSON.stringify(manifest, null, 2) + '\n')
}
