import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const source = readFileSync(
  new URL('../src/components/FoldPreview.tsx', import.meta.url),
  'utf8',
)

test('static fold graph messaging distinguishes cuts from cycle constraints', () => {
  assert.match(source, /model\.kinematics\.reason === 'cut_material_components'/)
  assert.match(source, /cuts separated the paper into multiple components/)
  assert.match(source, /積層折りパネルで閉路姿勢をプレビュー・適用できます/)
  assert.match(source, /apply the cycle pose in the stacked-fold panel below/)
  assert.match(source, /\$\{staticGraphReasonNote\}.*collisionDescription/s)
})
