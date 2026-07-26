import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const source = readFileSync(
  new URL('../scripts/runtime-updater-browser-e2e.mjs', import.meta.url),
  'utf8',
)

test('runtime updater E2E verifies its terminal state without a swallowed locator timeout', () => {
  assert.match(
    source,
    /Update application confirmed'\)\.waitFor\(\)[\s\S]*?getByRole\('button'\)\.count\(\)/u,
  )
  assert.match(source, /remainingButtonCount !== 0/u)
  assert.match(source, /runtime updater controls remained after explicit apply/u)
  assert.doesNotMatch(
    source,
    /getByRole\('button'\)\.focus\(\)\.catch\(/u,
  )
})
