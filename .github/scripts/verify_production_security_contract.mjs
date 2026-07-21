import { readdirSync, readFileSync, lstatSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { buildDependencyPolicy } from './dependency_policy.mjs'

const root = resolve(import.meta.dirname, '..', '..')
const dist = resolve(process.argv[2] ?? join(root, 'apps/desktop/dist'))
const config = JSON.parse(readFileSync(join(root, 'apps/desktop/src-tauri/tauri.conf.json'), 'utf8'))
const capability = JSON.parse(readFileSync(join(root, 'apps/desktop/src-tauri/capabilities/default.json'), 'utf8'))
const expectedPermissions = ['core:event:allow-listen', 'core:event:allow-unlisten', 'core:window:allow-destroy']
if (config.identifier !== 'dev.origami2.editor' || config.build?.frontendDist !== '../dist') throw new Error('production Tauri identity is invalid')
const csp = config.app?.security?.csp
const unapprovedCsp = typeof csp === 'string'
  ? csp.replaceAll('https://api.github.com', '').replaceAll('http://ipc.localhost', '')
  : ''
if (typeof csp !== 'string' || !csp.includes("default-src 'self'") || !csp.includes("object-src 'none'")
  || !csp.includes("script-src 'self'") || /unsafe-eval|\*|https?:\/\//u.test(unapprovedCsp)) {
  throw new Error('production CSP authority is broader than audited')
}
if (capability.identifier !== 'main-window' || JSON.stringify(capability.windows) !== '["main"]'
  || JSON.stringify(capability.permissions) !== JSON.stringify(expectedPermissions)) {
  throw new Error('production Tauri capability allowlist changed')
}
let files = 0
for (const name of readdirSync(dist, { recursive: true })) {
  const path = join(dist, name)
  const stat = lstatSync(path)
  if (stat.isSymbolicLink()) throw new Error('production bundle contains a symlink')
  if (!stat.isFile()) continue
  files += 1
  if (files > 4096 || stat.size > 16_777_216) throw new Error('production bundle scan bound exceeded')
  const bytes = readFileSync(path, 'utf8')
  if (/-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----|gh[opusr]_[A-Za-z0-9]{32,}|github_pat_[A-Za-z0-9_]{40,}|AKIA[0-9A-Z]{16}|(?:secret|token|password)\s*[:=]\s*["'][A-Za-z0-9+/_=-]{24,}["']/u.test(bytes)) {
    throw new Error(`production bundle contains a secret-like credential: ${name}`)
  }
}
if (files < 3) throw new Error('production bundle inventory is incomplete')
const policy = buildDependencyPolicy()
if (policy.result !== 'pass' || policy.npmPackages < 1 || policy.cargoPackages < 1
  || policy.thirdPartyNotices.length !== policy.npmPackages
  || policy.cargoLicenseDatabase.packages.length !== policy.cargoPackages) {
  throw new Error('dependency license inventory is incomplete')
}
process.stdout.write('production CSP, permissions, secret scan, and dependency licenses verified\n')
