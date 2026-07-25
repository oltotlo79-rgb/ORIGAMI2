import assert from 'node:assert/strict'
import test from 'node:test'

import {
  appendNamedTechniqueInstructionSteps,
  NamedTechniqueTimelineClientError,
  type NamedTechniqueTimelineProposalV1,
  type ProjectOccGuard,
} from '../src/lib/coreClient.ts'

const PROJECT_ID = '10000000-0000-4000-8000-000000000001'
const PROJECT_INSTANCE_ID = '20000000-0000-4000-8000-000000000002'
const GUARD: ProjectOccGuard = {
  expectedProjectInstanceId: PROJECT_INSTANCE_ID,
  expectedProjectId: PROJECT_ID,
  expectedRevision: 0,
}

test('rejects unknown fields and every hard-limit overflow before native invocation', async () => {
  const valid = proposal()
  const oversizedStepCount = {
    ...valid,
    steps: Array.from({ length: 513 }, () => valid.steps[0]),
  }
  const cases = [
    { ...valid, unknown: true },
    {
      ...valid,
      steps: [{ ...valid.steps[0], unknown: true }],
    },
    {
      ...valid,
      steps: [{
        ...valid.steps[0],
        description: 'x'.repeat(4_001),
      }],
    },
    oversizedStepCount,
  ]

  for (const candidate of cases) {
    await assert.rejects(
      appendNamedTechniqueInstructionSteps(
        GUARD,
        candidate as NamedTechniqueTimelineProposalV1,
      ),
      isInvalidRequest,
    )
  }
})

test('rejects accessors, sparse arrays, and hostile proxies without evaluating payload code', async () => {
  let getterCalls = 0
  const accessorStep = Object.create(null)
  for (const [key, value] of Object.entries(proposal().steps[0])) {
    Object.defineProperty(accessorStep, key, key === 'title'
      ? {
          enumerable: true,
          get() {
            getterCalls += 1
            return value
          },
        }
      : { enumerable: true, value })
  }
  const accessorArray: unknown[] = []
  Object.defineProperty(accessorArray, '0', {
    enumerable: true,
    get() {
      getterCalls += 1
      return proposal().steps[0]
    },
  })
  accessorArray.length = 1
  let proxyTrapCalls = 0
  const hostileProxy = new Proxy({}, {
    ownKeys() {
      proxyTrapCalls += 1
      throw new Error('C:\\private\\named-technique.json')
    },
  })
  const sparseSteps = new Array(1)

  for (const candidate of [
    { ...proposal(), steps: [accessorStep] },
    { ...proposal(), steps: accessorArray },
    { ...proposal(), steps: sparseSteps },
    hostileProxy,
  ]) {
    await assert.rejects(
      appendNamedTechniqueInstructionSteps(
        GUARD,
        candidate as NamedTechniqueTimelineProposalV1,
      ),
      (error: unknown) => {
        assert.ok(isInvalidRequest(error))
        assert.doesNotMatch(
          error instanceof Error ? error.message : String(error),
          /private|named-technique/u,
        )
        return true
      },
    )
  }
  assert.equal(getterCalls, 0)
  assert.equal(proxyTrapCalls, 1)
})

test('rejects primitive, accessor, proxy, and invalid OCC guards without reading later fields', async () => {
  let getterCalls = 0
  const accessorGuard = Object.create(null)
  Object.defineProperty(accessorGuard, 'expectedProjectInstanceId', {
    enumerable: true,
    get() {
      getterCalls += 1
      return PROJECT_INSTANCE_ID
    },
  })
  let proxyTrapCalls = 0
  const hostileGuard = new Proxy({}, {
    getOwnPropertyDescriptor() {
      proxyTrapCalls += 1
      throw new Error('C:\\private\\project-occ.json')
    },
  })
  let laterFieldCalls = 0
  const invalidFirst = {
    expectedProjectInstanceId: 'invalid',
    get expectedProjectId() {
      laterFieldCalls += 1
      return PROJECT_ID
    },
    get expectedRevision() {
      laterFieldCalls += 1
      return 0
    },
  }
  const invalidSecond = {
    expectedProjectInstanceId: PROJECT_INSTANCE_ID,
    expectedProjectId: 'invalid',
    get expectedRevision() {
      laterFieldCalls += 1
      return 0
    },
  }
  const invalidTuples = [
    { ...GUARD, expectedRevision: -1 },
    { ...GUARD, expectedRevision: 0.5 },
    { ...GUARD, expectedRevision: Number.MAX_SAFE_INTEGER },
  ]

  for (const candidate of [
    null,
    undefined,
    0,
    'guard',
    accessorGuard,
    hostileGuard,
    invalidFirst,
    invalidSecond,
    ...invalidTuples,
  ]) {
    await assert.rejects(
      appendNamedTechniqueInstructionSteps(
        candidate as ProjectOccGuard,
        proposal(),
      ),
      (error: unknown) => {
        assert.ok(isInvalidRequest(error))
        assert.doesNotMatch(
          error instanceof Error ? error.message : String(error),
          /private|project-occ/u,
        )
        return true
      },
    )
  }
  assert.equal(getterCalls, 0)
  assert.equal(proxyTrapCalls, 1)
  assert.equal(laterFieldCalls, 0)
})

function proposal(): NamedTechniqueTimelineProposalV1 {
  return {
    schema_version: 1,
    package_id: 'builtin.origami2',
    technique_id: 'inside-reverse',
    technique_version: 1,
    steps: [{
      source_kind: 'technique',
      source_id: 'inside-reverse',
      chunk_index: 1,
      chunk_count: 1,
      title: 'Inside reverse fold',
      description: 'source-json-v1:\n{}',
      caution: 'Description only',
      duration_ms: 1_500,
    }],
  }
}

function isInvalidRequest(error: unknown) {
  return error instanceof NamedTechniqueTimelineClientError
    && error.code === 'invalid_request'
}
