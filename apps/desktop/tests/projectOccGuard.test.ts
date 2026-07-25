import assert from 'node:assert/strict'
import test from 'node:test'

import {
  matchesProjectOccGuard,
  type ProjectOccGuard,
} from '../src/lib/coreClient.ts'

const guard: ProjectOccGuard = {
  expectedProjectInstanceId: 'instance-1',
  expectedProjectId: 'project-1',
  expectedRevision: 7,
}
const project = {
  project_instance_id: 'instance-1',
  project_id: 'project-1',
  revision: 7,
}

test('ProjectOccGuard requires the exact instance, project, and revision tuple', () => {
  assert.equal(matchesProjectOccGuard(guard, project), true)
  assert.equal(matchesProjectOccGuard(guard, {
    ...project,
    project_instance_id: 'instance-2',
  }), false)
  assert.equal(matchesProjectOccGuard(guard, {
    ...project,
    project_id: 'project-2',
  }), false)
  assert.equal(matchesProjectOccGuard(guard, {
    ...project,
    revision: 8,
  }), false)
})

test('ProjectOccGuard fails closed on accessors and proxies in comparison order', () => {
  let laterReads = 0
  const invalidInstanceGuard = {
    expectedProjectInstanceId: 'instance-2',
    get expectedProjectId() {
      laterReads += 1
      return 'project-1'
    },
    get expectedRevision() {
      laterReads += 1
      return 7
    },
  }
  assert.equal(matchesProjectOccGuard(
    invalidInstanceGuard as ProjectOccGuard,
    project,
  ), false)
  assert.equal(laterReads, 0)

  let getterCalls = 0
  const accessorProject = Object.create(null)
  Object.defineProperty(accessorProject, 'project_instance_id', {
    get() {
      getterCalls += 1
      return 'instance-1'
    },
  })
  assert.equal(matchesProjectOccGuard(guard, accessorProject), false)
  assert.equal(getterCalls, 0)

  let proxyCalls = 0
  const hostileProject = new Proxy({}, {
    getOwnPropertyDescriptor() {
      proxyCalls += 1
      throw new Error('private path')
    },
  })
  assert.equal(matchesProjectOccGuard(guard, hostileProject as typeof project), false)
  assert.equal(proxyCalls, 1)
})
