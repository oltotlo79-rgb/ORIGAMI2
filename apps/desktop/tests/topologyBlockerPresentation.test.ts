import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const appSource = readFileSync(new URL('../src/App.tsx', import.meta.url), 'utf8')

test('3D transition blockers expose every reason and every related geometry location', () => {
  assert.match(appSource, /topologyResponse\.issues\.map/)
  assert.doesNotMatch(appSource, /topologyResponse\.issues\.slice/)
  assert.match(appSource, /topologyIssueLabel\(issue\.kind, locale\)/)
  assert.match(appSource, /topologyIssueLocations\(issue\.kind\)/)
  assert.match(appSource, /too_many_active_fold_edges[\s\S]*issue\.edges\.map/)
  assert.match(appSource, /fold_endpoint_not_on_boundary[\s\S]*issue\.vertex/)
  assert.match(appSource, /setSelectedLineId\(location\.id\)/)
  assert.match(appSource, /setSelectedVertexId\(location\.id\)/)
})

