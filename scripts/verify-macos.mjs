import assert from 'node:assert/strict'
import { existsSync } from 'node:fs'
import { join } from 'node:path'
import { spawnSync, spawn } from 'node:child_process'

assert.equal(process.platform, 'darwin')
const app = process.env.APP_PATH
const arch = process.env.EXPECTED_MACHINE
assert.ok(app && arch)
const executable = join(app, 'Contents/MacOS/cloudloom')
const engine = join(app, 'Contents/Resources/resources/rclone')
for (const path of [executable, engine, join(app, 'Contents/Resources/resources/RCLONE-LICENSE.txt')]) {
  assert.ok(existsSync(path), `Missing packaged file: ${path}`)
  if (!path.endsWith('.txt')) {
    const architecture = spawnSync('/usr/bin/lipo', ['-archs', path], { encoding: 'utf8' })
    assert.equal(architecture.status, 0, architecture.stderr)
    assert.ok(architecture.stdout.trim().split(/\s+/).includes(arch), `Wrong architecture: ${architecture.stdout}`)
  }
}
const signature = spawnSync('/usr/bin/codesign', ['--verify', '--deep', '--strict', '--verbose=2', app], { encoding: 'utf8' })
assert.equal(signature.status, 0, signature.stderr)
const version = spawnSync(engine, ['version'], { encoding: 'utf8' })
assert.equal(version.status, 0, version.stderr)
assert.ok(version.stdout.startsWith('rclone v1.75.2\n'))
console.log(version.stdout)
// A runner launch checks startup/resource/Keychain initialization, not account sign-in or GUI interaction.
const child = spawn(executable, [], { stdio: ['ignore', 'pipe', 'pipe'] })
let diagnostics = ''
child.stdout.on('data', data => { diagnostics += data.toString() })
child.stderr.on('data', data => { diagnostics += data.toString() })
const startup = await Promise.race([
  new Promise(resolve => child.once('exit', (code, signal) => resolve({ code, signal }))),
  new Promise(resolve => setTimeout(() => resolve(null), 12000)),
])
if (startup !== null) throw new Error(`Packaged app exited during startup: ${JSON.stringify(startup)}\n${diagnostics}`)
child.kill('SIGTERM')
console.log('Packaged app stayed running during startup smoke test. Developer ID signature/notarization is NOT present.')
