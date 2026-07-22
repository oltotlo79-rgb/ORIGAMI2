import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const app = readFileSync('src/App.tsx', 'utf8')
const preview = readFileSync('src/components/FoldPreview.tsx', 'utf8')
const panel = readFileSync('src/components/InstructionTimelinePanel.tsx', 'utf8')

test('captured instruction cameras stay bound to the exact preview model', () => {
  assert.match(
    app,
    /setFoldPreviewCamera\(\{ poseModelKey: foldPreviewPoseModelKey, camera \}\)/u,
  )
  assert.match(
    app,
    /currentCamera=\{foldPreviewCamera\?\.poseModelKey === foldPreviewPoseModelKey[\s\S]*?foldPreviewCamera\.camera[\s\S]*?: null\}/u,
  )
  assert.match(preview, /onCameraChangeRef\.current\?\.\(\{/u)
  assert.match(preview, /createdControls\.addEventListener\('change', controlsChangeHandler\)[\s\S]*?controlsChangeHandler\(\)/u)
  assert.match(panel, /disabled=\{editingDisabled \|\| !currentCamera\}/u)
})
