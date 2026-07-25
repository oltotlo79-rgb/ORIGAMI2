import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const app = read('../src/App.tsx')

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

function read(relativePath: string) {
  return readFileSync(new URL(relativePath, import.meta.url), 'utf8')
}
