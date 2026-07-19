import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const mainSource = readFileSync(
  new URL('../src/main.tsx', import.meta.url),
  'utf8',
)
const htmlSource = readFileSync(
  new URL('../index.html', import.meta.url),
  'utf8',
)

test('locale initializes before React mounts and is disposed during HMR', () => {
  const initialization = mainSource.indexOf('initializeLocaleStore()')
  const mount = mainSource.indexOf('createRoot(')

  assert.ok(initialization >= 0)
  assert.ok(mount >= 0)
  assert.ok(initialization < mount)
  assert.match(mainSource, /localeStore\.dispose\(\)/u)
})

test('the static document language matches the Japanese default', () => {
  assert.match(htmlSource, /<html\s+lang="ja">/u)
  assert.doesNotMatch(htmlSource, /<html\s+lang="en">/u)
})
