import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'
import { ORIGAMI2_GITHUB_RELEASES_API_URL } from '../src/lib/githubReleaseUpdate.ts'

test('the desktop CSP permits only the fixed GitHub API update authority', async () => {
  const configText = await readFile(
    new URL('../src-tauri/tauri.conf.json', import.meta.url),
    'utf8',
  )
  const config: unknown = JSON.parse(configText)
  assert.ok(isRecord(config))
  assert.ok(isRecord(config.app))
  assert.ok(isRecord(config.app.security))
  assert.equal(typeof config.app.security.csp, 'string')

  const directives = parseCsp(config.app.security.csp)
  assert.deepEqual(directives.get('connect-src'), [
    "'self'",
    'ipc:',
    'http://ipc.localhost',
    'https://api.github.com',
  ])
  assert.deepEqual(directives.get('default-src'), ["'self'"])
  assert.deepEqual(directives.get('script-src'), ["'self'"])
  assert.deepEqual(directives.get('object-src'), ["'none'"])
  assert.deepEqual(directives.get('base-uri'), ["'none'"])
  assert.deepEqual(directives.get('frame-ancestors'), ["'none'"])
  assert.equal(config.app.security.csp.includes("'unsafe-eval'"), false)
  assert.equal(config.app.security.csp.includes('localhost'), true)
  assert.equal(config.app.security.csp.includes('http://localhost'), false)
  assert.equal(config.app.security.csp.includes('ws://'), false)
  assert.equal(config.app.security.csp.includes('wss://'), false)
  assert.equal(config.app.security.csp.includes('*.github.com'), false)
  assert.equal(config.app.security.csp.includes('https://github.com'), false)
  assert.equal(config.app.security.csp.includes('http://api.github.com'), false)
})

test('production update authority cannot be widened by origin credentials DNS aliases or dev proxy', async () => {
  const endpoint = new URL(ORIGAMI2_GITHUB_RELEASES_API_URL)
  assert.equal(endpoint.protocol, 'https:')
  assert.equal(endpoint.hostname, 'api.github.com')
  assert.equal(endpoint.host, 'api.github.com')
  assert.equal(endpoint.username, '')
  assert.equal(endpoint.password, '')
  assert.equal(endpoint.port, '')
  assert.equal(endpoint.search, '')
  assert.equal(endpoint.hash, '')
  assert.equal(
    endpoint.pathname,
    '/repos/oltotlo79-rgb/ORIGAMI2/releases/latest',
  )

  const tauriConfig = JSON.parse(await readFile(
    new URL('../src-tauri/tauri.conf.json', import.meta.url),
    'utf8',
  )) as { build?: { devUrl?: unknown; frontendDist?: unknown } }
  assert.equal(tauriConfig.build?.devUrl, 'http://localhost:1420')
  assert.equal(tauriConfig.build?.frontendDist, '../dist')

  const viteConfig = await readFile(
    new URL('../vite.config.ts', import.meta.url),
    'utf8',
  )
  assert.match(viteConfig, /host: '127\.0\.0\.1'/u)
  assert.match(viteConfig, /strictPort: true/u)
  assert.doesNotMatch(viteConfig, /proxy\s*:/u)
  assert.doesNotMatch(viteConfig, /changeOrigin|rewrite|api\.github\.com/iu)
  const serializedTauriConfig = JSON.stringify(tauriConfig)
  assert.doesNotMatch(serializedTauriConfig, /"devtools"\s*:\s*true/iu)
  assert.doesNotMatch(serializedTauriConfig, /dangerousDisableAssetCspModification/iu)
})

function parseCsp(value: string): Map<string, string[]> {
  const directives = new Map<string, string[]>()
  for (const source of value.split(';')) {
    const tokens = source.trim().split(/\s+/u)
    const name = tokens.shift()
    if (!name) continue
    assert.equal(directives.has(name), false)
    directives.set(name, tokens)
  }
  return directives
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}
