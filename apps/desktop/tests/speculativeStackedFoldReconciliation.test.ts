import assert from 'node:assert/strict'
import test from 'node:test'

import {
  reconcileSpeculativeStackedFoldApplyV1,
} from '../src/lib/speculativeStackedFoldReconciliation.ts'
import {
  makeSpeculativeSnapshot,
  SPECULATIVE_INSTANCE_ID,
  SPECULATIVE_PROJECT_ID,
} from './stackedFoldSpeculativeFixture.ts'

const authority = Object.freeze({
  projectInstanceId: SPECULATIVE_INSTANCE_ID,
  projectId: SPECULATIVE_PROJECT_ID,
  sourceRevision: 3,
  targetRevision: 4,
})

test('reconciles only exact unchanged or single-commit project revisions', async () => {
  const committed = makeSpeculativeSnapshot()
  committed.revision = 4
  assert.deepEqual(await reconcileSpeculativeStackedFoldApplyV1(
    async () => committed,
    authority,
  ), {
    kind: 'committed',
    snapshot: committed,
  })
  assert.deepEqual(await reconcileSpeculativeStackedFoldApplyV1(
    async () => makeSpeculativeSnapshot(),
    authority,
  ), { kind: 'unchanged' })

  for (const snapshot of [
    { ...makeSpeculativeSnapshot(), revision: 5 },
    {
      ...makeSpeculativeSnapshot(),
      project_instance_id: '018f47a2-4b7a-7cc1-8abc-aabbccddeeff',
      revision: 4,
    },
    {
      ...makeSpeculativeSnapshot(),
      project_id: '018f47a2-4b7a-7cc1-8abc-aabbccddeeff',
      revision: 4,
    },
    { ...makeSpeculativeSnapshot(), revision: 4.000000000000001 },
  ]) {
    assert.deepEqual(await reconcileSpeculativeStackedFoldApplyV1(
      async () => snapshot,
      authority,
    ), { kind: 'unavailable' })
  }
})

test('contains refresh failures, accessors, inherited values, and hostile Proxies', async () => {
  assert.deepEqual(await reconcileSpeculativeStackedFoldApplyV1(
    async () => {
      throw new Error('C:\\private\\project.ori2')
    },
    authority,
  ), { kind: 'unavailable' })

  let getterCalls = 0
  const accessor = makeSpeculativeSnapshot()
  Object.defineProperty(accessor, 'revision', {
    enumerable: true,
    get() {
      getterCalls += 1
      throw new Error('private native detail')
    },
  })
  assert.deepEqual(await reconcileSpeculativeStackedFoldApplyV1(
    async () => accessor,
    authority,
  ), { kind: 'unavailable' })
  assert.equal(getterCalls, 0)

  assert.deepEqual(await reconcileSpeculativeStackedFoldApplyV1(
    async () => Object.create({
      ...makeSpeculativeSnapshot(),
      revision: 4,
    }),
    authority,
  ), { kind: 'unavailable' })

  const proxyGetKeys: PropertyKey[] = []
  const target = { ...makeSpeculativeSnapshot(), revision: 4 }
  const proxied = new Proxy(target, {
    get(_target, key) {
      proxyGetKeys.push(key)
      if (key === 'then') return undefined
      throw new Error('private native detail')
    },
  })
  assert.equal(
    (await reconcileSpeculativeStackedFoldApplyV1(
      async () => proxied,
      authority,
    )).kind,
    'committed',
  )
  // Await performs the language-defined thenability check. Project binding
  // fields still cross only through own data descriptors.
  assert.deepEqual(proxyGetKeys, ['then'])

  const revocable = Proxy.revocable(target, {})
  revocable.revoke()
  assert.deepEqual(await reconcileSpeculativeStackedFoldApplyV1(
    async () => revocable.proxy,
    authority,
  ), { kind: 'unavailable' })
})

test('rejects malformed expected authority without refreshing', async () => {
  let refreshCalls = 0
  for (const expected of [
    { ...authority, projectId: 'not-a-project' },
    { ...authority, targetRevision: 5 },
    { ...authority, sourceRevision: -0, targetRevision: 1 },
    Object.create(authority),
  ]) {
    assert.deepEqual(await reconcileSpeculativeStackedFoldApplyV1(
      async () => {
        refreshCalls += 1
        return makeSpeculativeSnapshot()
      },
      expected,
    ), { kind: 'unavailable' })
  }
  assert.equal(refreshCalls, 0)
})
