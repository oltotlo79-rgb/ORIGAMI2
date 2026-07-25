import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const app = read('../src/App.tsx')
const dialog = read('../src/components/MeshAnimationExportDialog.tsx')

test('animation export route revalidates binding and rejects stale responses', () => {
  assert.match(
    app,
    /!matchesProjectOccGuard\(\{\s*expectedProjectInstanceId: preview\.projectInstanceId,\s*expectedProjectId: preview\.projectId,\s*expectedRevision: preview\.revision,\s*\}, latest\)/u,
  )
  assert.match(app, /cancelInstructionMeshAnimation\(preview\.exportId\)/u)
})

test('animation export route closes reentry and disposal generations', () => {
  assert.match(app, /coreOperationRef\.current/u)
  assert.match(app, /\+\+meshAnimationExportRequestIdRef\.current/u)
  assert.match(app, /requestId !== meshAnimationExportRequestIdRef\.current/u)
  assert.match(app, /\|\| meshAnimationExportOpen/u)
})

test('animation export dialog keeps locale copy and formatting out of the component', () => {
  assert.match(dialog, /MESH_ANIMATION_EXPORT_DIALOG_TEXT as COPY/u)
  assert.doesNotMatch(dialog, /locale\s*===|locale\s*!==/u)
  assert.doesNotMatch(dialog, /'vertices'|'triangles'|'bytes'|'\\u00a0'/u)
})

function read(relativePath: string) {
  return readFileSync(new URL(relativePath, import.meta.url), 'utf8')
}
