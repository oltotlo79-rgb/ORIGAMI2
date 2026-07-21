import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { dirname, join, resolve } from 'node:path'

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
