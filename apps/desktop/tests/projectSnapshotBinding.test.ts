import assert from 'node:assert/strict'
import test from 'node:test'

import { isExpectedNativeEditSnapshot } from '../src/lib/projectSnapshotBinding.ts'

const INSTANCE = '1aaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1'
const PROJECT = '2bbbbbbb-bbbb-4bbb-9bbb-bbbbbbbbbbb2'

test('accepts only the exact project instance and next safe revision', () => {
  const result = {
    project_instance_id: INSTANCE,
    project_id: PROJECT,
    revision: 42,
    unrelated: 'snapshot fields remain feature-admitted',
  }

  assert.equal(isExpectedNativeEditSnapshot(result, INSTANCE, PROJECT, 41), true)
  for (const invalid of [
    { ...result, project_instance_id: '30000000-0000-4000-8000-000000000003' },
    { ...result, project_id: '30000000-0000-4000-8000-000000000003' },
    { ...result, revision: 41 },
    { ...result, revision: 43 },
    { ...result, revision: 42.5 },
    null,
    [],
  ]) {
    assert.equal(isExpectedNativeEditSnapshot(invalid, INSTANCE, PROJECT, 41), false)
  }
  assert.equal(
    isExpectedNativeEditSnapshot(result, INSTANCE.toUpperCase(), PROJECT, 41),
    false,
  )
  assert.equal(
    isExpectedNativeEditSnapshot(
      { ...result, revision: Number.MAX_SAFE_INTEGER },
      INSTANCE,
      PROJECT,
      Number.MAX_SAFE_INTEGER,
    ),
    false,
  )
})

test('rejects accessors, foreign prototypes, and hostile proxies without invoking values', () => {
  let getterCalls = 0
  const accessor = {
    project_id: PROJECT,
    revision: 42,
  }
  Object.defineProperty(accessor, 'project_instance_id', {
    enumerable: true,
    get() {
      getterCalls += 1
      return INSTANCE
    },
  })
  assert.equal(isExpectedNativeEditSnapshot(accessor, INSTANCE, PROJECT, 41), false)
  assert.equal(getterCalls, 0)

  assert.equal(
    isExpectedNativeEditSnapshot(
      Object.assign(Object.create({}), {
        project_instance_id: INSTANCE,
        project_id: PROJECT,
        revision: 42,
      }),
      INSTANCE,
      PROJECT,
      41,
    ),
    false,
  )
  assert.equal(
    isExpectedNativeEditSnapshot(
      new Proxy({}, {
        ownKeys() {
          throw new Error('private trap detail')
        },
      }),
      INSTANCE,
      PROJECT,
      41,
    ),
    false,
  )
})
