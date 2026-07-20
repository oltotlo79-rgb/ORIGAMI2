import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const app = read('../src/App.tsx')

test('animation export route revalidates binding and rejects stale responses', () => {
  assert.match(app, /latest\.project_instance_id !== preview\.projectInstanceId/u)
  assert.match(app, /latest\.project_id !== preview\.projectId/u)
  assert.match(app, /latest\.revision !== preview\.revision/u)
  assert.match(app, /cancelInstructionMeshAnimation\(preview\.exportId\)/u)
})

test('animation export route closes reentry and disposal generations', () => {
  assert.match(app, /coreOperationRef\.current/u)
  assert.match(app, /\+\+meshAnimationExportRequestIdRef\.current/u)
  assert.match(app, /requestId !== meshAnimationExportRequestIdRef\.current/u)
  assert.match(app, /\|\| meshAnimationExportOpen/u)
})

function read(relativePath: string) {
  return readFileSync(new URL(relativePath, import.meta.url), 'utf8')
}
