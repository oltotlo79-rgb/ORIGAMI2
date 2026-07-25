import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  FOLD_PREVIEW_COMPONENT_TEXT as TEXT,
} from '../src/lib/foldPreviewComponentText.ts'

const source = readFileSync(
  new URL('../src/components/FoldPreview.tsx', import.meta.url),
  'utf8',
)

test('static fold graph messaging distinguishes cuts from cycle constraints', () => {
  assert.match(source, /model\.kinematics\.reason === 'cut_material_components'/)
  assert.match(
    TEXT.cutComponentsPlanarOnly.en,
    /cuts separated the paper into multiple components/,
  )
  assert.match(
    TEXT.cycleConstraintsPlanarOnly.ja,
    /積層折りパネルで閉路姿勢をプレビュー・適用できます/,
  )
  assert.match(
    TEXT.cycleConstraintsPlanarOnly.en,
    /apply the cycle pose in the stacked-fold panel below/,
  )
  assert.match(source, /reason: staticGraphReasonNote,[\s\S]*collision: collisionDescription/u)
})
