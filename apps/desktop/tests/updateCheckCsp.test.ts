import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

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
  assert.equal(config.app.security.csp.includes('*.github.com'), false)
  assert.equal(config.app.security.csp.includes('https://github.com'), false)
  assert.equal(config.app.security.csp.includes('http://api.github.com'), false)
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
