import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { dirname, join, resolve } from 'node:path'
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..')
const forbidden = [
  'WINDOWS_CERTIFICATE_BASE64',
  'WINDOWS_CERTIFICATE_PASSWORD',
  'APPLE_CERTIFICATE_BASE64',
  'APPLE_CERTIFICATE_PASSWORD',
  'APPLE_SIGNING_IDENTITY',
  'APPLE_NOTARY_ISSUER_ID',
  'APPLE_NOTARY_KEY_ID',
  'APPLE_NOTARY_KEY_BASE64',
  'RELEASE_SIGNING_PUBLIC_KEY',
]
for (const name of forbidden) {
  if (process.env[name]) throw new Error(`credential-free release dry-run forbids ${name}`)
}
if (process.env.GITHUB_REF?.startsWith('refs/tags/')) {
  throw new Error('release dry-run refuses tag refs')
}

const compatibility = spawnSync(
  process.execPath,
  [join(root, '.github/scripts/verify_update_compatibility_fixture.mjs')],
  { cwd: root, env: process.env, encoding: 'utf8' },
)
process.stdout.write(compatibility.stdout ?? '')
process.stderr.write(compatibility.stderr ?? '')
if (compatibility.status !== 0) process.exit(compatibility.status ?? 1)

const runtimeCompatibility = spawnSync(
  process.execPath,
  [join(root, '.github/scripts/verify_runtime_updater_release_fixture.mjs')],
  { cwd: root, env: process.env, encoding: 'utf8' },
)
process.stdout.write(runtimeCompatibility.stdout ?? '')
process.stderr.write(runtimeCompatibility.stderr ?? '')
if (runtimeCompatibility.status !== 0) process.exit(runtimeCompatibility.status ?? 1)

const signedCandidate = spawnSync(
  process.execPath,
  [join(root, '.github/scripts/rehearse_release_candidate.mjs')],
  { cwd: root, env: process.env, encoding: 'utf8' },
)
process.stdout.write(signedCandidate.stdout ?? '')
process.stderr.write(signedCandidate.stderr ?? '')
if (signedCandidate.status !== 0) process.exit(signedCandidate.status ?? 1)

const smokeRoot = mkdtempSync(join(tmpdir(), 'origami2-release-smoke-'))
try {
  for (const directory of [
    'windows/resources',
    'macos/ORIGAMI2.app/Contents/MacOS',
    'macos/ORIGAMI2.app/Contents/Resources',
  ]) mkdirSync(join(smokeRoot, directory), { recursive: true })
  const launcher = "if(process.env.ORIGAMI2_NETWORK_DISABLED!=='1')process.exit(9);console.log('ORIGAMI2_SMOKE_OK')\n"
  for (const target of [
    'windows/portable.mock.js',
    'windows/installer.mock.js',
    'macos/ORIGAMI2.app/Contents/MacOS/ORIGAMI2.mock.js',
  ]) writeFileSync(join(smokeRoot, target), launcher)
  writeFileSync(join(smokeRoot, 'windows/resources/app.asar'), 'bounded offline resource')
  writeFileSync(join(smokeRoot, 'macos/ORIGAMI2.app/Contents/Resources/app.asar'), 'bounded offline resource')
  writeFileSync(join(smokeRoot, 'windows/installer-manifest.json'), JSON.stringify({
    networkAuthorities: [], resources: ['resources/app.asar'],
    uninstall: { displayName: 'ORIGAMI2', quietCommand: 'uninstall.exe /S' },
  }))
  writeFileSync(join(smokeRoot, 'macos/ORIGAMI2.app/Contents/Info.plist'),
    '<plist><dict><key>CFBundleIdentifier</key><string>com.origami2.desktop</string><key>CFBundleExecutable</key><string>ORIGAMI2.mock.js</string></dict></plist>')
  const smoke = spawnSync(process.execPath, [join(root, '.github/scripts/verify_release_smoke_fixture.mjs'), smokeRoot], {
    cwd: root, env: process.env, encoding: 'utf8',
  })
  process.stdout.write(smoke.stdout ?? '')
  process.stderr.write(smoke.stderr ?? '')
  if (smoke.status !== 0) process.exit(smoke.status ?? 1)
} finally {
  rmSync(smokeRoot, { recursive: true, force: true })
}

const result = spawnSync(
  process.execPath,
  [
    '--test',
    '--test-name-pattern=credential-free dry-run fixture proves the complete nine-asset handoff',
    join(root, '.github', 'tests', 'formal-release.test.mjs'),
  ],
  { cwd: root, env: process.env, encoding: 'utf8' },
)
process.stdout.write(result.stdout ?? '')
process.stderr.write(result.stderr ?? '')
if (result.status !== 0) process.exit(result.status ?? 1)
process.stdout.write('credential-free Windows/macOS release artifact dry-run verified\n')
