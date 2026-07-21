import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, relative, resolve, sep } from 'node:path'
import { spawnSync } from 'node:child_process'

const root = resolve(process.argv[2] ?? '')
if (!process.argv[2] || root === resolve(sep)) throw new Error('invalid smoke fixture root')
const files = (directory) => readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
  const path = join(directory, entry.name)
  return entry.isDirectory() ? files(path) : [relative(root, path).replaceAll('\\', '/')]
})
const expected = [
  'macos/ORIGAMI2.app/Contents/Info.plist',
  'macos/ORIGAMI2.app/Contents/MacOS/ORIGAMI2.mock.js',
  'macos/ORIGAMI2.app/Contents/Resources/app.asar',
  'windows/installer-manifest.json',
  'windows/installer.mock.js',
  'windows/portable.mock.js',
  'windows/resources/app.asar',
].sort()
const actual = files(root).sort()
if (JSON.stringify(actual) !== JSON.stringify(expected)) throw new Error('smoke fixture has missing or extra assets')
for (const name of actual) {
  const path = resolve(root, name)
  if (!path.startsWith(`${root}${sep}`) || !statSync(path).isFile()) throw new Error('smoke fixture path escaped root')
}
const manifest = JSON.parse(readFileSync(join(root, 'windows/installer-manifest.json'), 'utf8'))
if (JSON.stringify(Object.keys(manifest).sort()) !== JSON.stringify(['networkAuthorities', 'resources', 'uninstall'].sort()) ||
    manifest.networkAuthorities.length !== 0 || manifest.resources.join(',') !== 'resources/app.asar' ||
    manifest.uninstall.displayName !== 'ORIGAMI2' || manifest.uninstall.quietCommand !== 'uninstall.exe /S') {
  throw new Error('Windows installer or uninstall metadata is invalid')
}
const plist = readFileSync(join(root, 'macos/ORIGAMI2.app/Contents/Info.plist'), 'utf8')
if (!plist.includes('<string>com.origami2.desktop</string>') || !plist.includes('<string>ORIGAMI2.mock.js</string>')) {
  throw new Error('macOS bundle metadata is invalid')
}
for (const target of ['windows/portable.mock.js', 'windows/installer.mock.js', 'macos/ORIGAMI2.app/Contents/MacOS/ORIGAMI2.mock.js']) {
  const launched = spawnSync(process.execPath, [join(root, target)], {
    env: { PATH: process.env.PATH ?? '', ORIGAMI2_NETWORK_DISABLED: '1' }, encoding: 'utf8', timeout: 5_000,
  })
  if (launched.status !== 0 || launched.stdout.trim() !== 'ORIGAMI2_SMOKE_OK') throw new Error(`launch smoke failed: ${target}`)
}
process.stdout.write('release bundle launch/resource/uninstall smoke fixture verified\n')
