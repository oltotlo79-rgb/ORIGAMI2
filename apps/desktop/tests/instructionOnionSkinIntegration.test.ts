import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const preview = readFileSync(new URL('../src/components/FoldPreview.tsx', import.meta.url), 'utf8')
const panel = readFileSync(new URL('../src/components/InstructionTimelinePanel.tsx', import.meta.url), 'utf8')

test('ghost material and geometry are privately owned read-only visual resources', () => {
  assert.match(preview, /new THREE\.MeshBasicMaterial\([\s\S]*transparent: true,[\s\S]*opacity:[\s\S]*depthWrite: false/u)
  assert.match(preview, /createFoldPreviewFaceGeometry\(face\.polygon[\s\S]*ghostGeometries\.push/u)
  assert.match(preview, /mesh\.raycast = \(\) => undefined/u)
  assert.match(preview, /for \(const geometry of ghostGeometries\)[\s\S]*geometry\.dispose/u)
  assert.match(preview, /for \(const material of ghostMaterials\)[\s\S]*material\.dispose/u)
})

test('ghost creation is not registered with pick selection collision or current pose callbacks', () => {
  const ghostBlock = preview.slice(preview.indexOf('const ghostTransforms'), preview.indexOf('const instructionVisualGroup'))
  assert.doesNotMatch(ghostBlock, /facePickObjects|vertexPickObjects|hingePickObjects|updateCollision|onSelect|onAppliedPose/u)
  assert.match(panel, /'off', 'previous', 'next'/u)
  assert.match(panel, /role="status" aria-live="polite"/u)
})
