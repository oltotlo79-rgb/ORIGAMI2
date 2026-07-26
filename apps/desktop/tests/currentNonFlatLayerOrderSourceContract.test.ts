import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const SOURCES = [
  'src/lib/currentNonFlatLayerOrderView.ts',
  'src/components/CurrentNonFlatLayerOrderViewer.tsx',
] as const

test('the non-flat layer-order viewer sources contain no literal NUL bytes', () => {
  for (const path of SOURCES) {
    const source = readFileSync(path)
    assert.equal(
      source.includes(0),
      false,
      `${path} must remain an ordinary reviewable text source`,
    )
  }
})
