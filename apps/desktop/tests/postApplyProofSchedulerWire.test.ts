import assert from 'node:assert/strict'
import test from 'node:test'

import {
  POST_APPLY_PROOF_STATUSES_V1,
  PostApplyProofSchedulerClientError,
  createRevertPostApplyProofFailureRequestV1,
  createPostApplyProofSchedulerClientV1,
  normalizePostApplyProofJobRequestV1,
  normalizePostApplyProofProgressV1,
  normalizeRevertPostApplyProofFailureRequestV1,
  normalizeStartPostApplyProofJobRequestV1,
} from '../src/lib/postApplyProofSchedulerClient.ts'

const projectInstanceId = '018f47a2-4b7a-7cc1-8abc-112233445566'
const projectId = '018f47a2-4b7a-7cc1-8abc-665544332211'
const jobToken = '018f47a2-4b7a-7cc1-8abc-778899aabbcc'

const binding = Object.freeze({
  version: 1 as const,
  projectInstanceId,
  projectId,
  revision: 8,
})

function progress(
  overrides: Readonly<Record<string, unknown>> = {},
): Readonly<Record<string, unknown>> {
  const value: Record<string, unknown> = {
    ...binding,
    jobToken,
    status: 'proving',
    provenPairCount: 0,
    totalPairCount: 5,
    proofFailure: null,
    ...overrides,
  }
  if (!Object.hasOwn(overrides, 'proofFailure')) {
    value.proofFailure = failureForStatus(value.status)
  }
  return value
}

function failureForStatus(status: unknown): Readonly<Record<string, unknown>> | null {
  const reason = status === 'blocked'
    ? null
    : status === 'unknown_evidence_insufficient'
      ? 'evidence_insufficient'
      : status === 'unknown_resource_limit'
        ? 'resource_limit'
        : status === 'unknown_cancelled'
          ? 'cancelled'
          : status === 'unknown_deadline_reached'
            ? 'deadline_reached'
            : undefined
  if (reason === undefined) return null
  return {
    location: 'applied_retained_undo',
    outcome: status === 'blocked' ? 'blocked' : 'unknown',
    reason,
    subsequentEditCount: 2,
    undoStepsToRevert: 3,
  }
}

test('post-Apply proof v1 accepts every coarse status and returns a detached frozen snapshot', () => {
  assert.deepEqual(POST_APPLY_PROOF_STATUSES_V1, [
    'proving',
    'certified',
    'blocked',
    'unknown_evidence_insufficient',
    'unknown_resource_limit',
    'unknown_cancelled',
    'unknown_deadline_reached',
    'stale',
  ])
  assert.equal(Object.isFrozen(POST_APPLY_PROOF_STATUSES_V1), true)
  for (const status of POST_APPLY_PROOF_STATUSES_V1) {
    const source = progress({
      status,
      ...(status === 'certified' ? { provenPairCount: 5 } : {}),
    })
    const normalized = normalizePostApplyProofProgressV1(source)
    const wireFailure = failureForStatus(status)
    assert.deepEqual(normalized, {
      ...source,
      proofFailure: wireFailure === null
        ? null
        : {
            location: wireFailure.location,
            reason: status === 'blocked'
              ? 'blocked'
              : status === 'unknown_deadline_reached'
                ? 'deadline'
                : wireFailure.reason,
            subsequentEditCount: wireFailure.subsequentEditCount,
            undoStepsToRevert: wireFailure.undoStepsToRevert,
          },
    })
    assert.equal(Object.isFrozen(normalized), true)
    assert.notEqual(normalized, source)
    assert.equal(normalizePostApplyProofProgressV1(normalized), normalized)
  }
})

test('atomic proof revert binds the current revision to the exact terminal report', async () => {
  const terminal = normalizePostApplyProofProgressV1(progress({
    status: 'unknown_deadline_reached',
  }))
  assert.notEqual(terminal, null)
  const failure = terminal!.proofFailure
  assert.notEqual(failure, null)
  const request = createRevertPostApplyProofFailureRequestV1(
    terminal!,
    11,
    failure!,
  )
  assert.deepEqual(request, {
    version: 1,
    projectInstanceId,
    projectId,
    expectedRevision: 11,
    jobToken,
    expectedLocation: 'applied_retained_undo',
    expectedOutcome: 'unknown',
    expectedReason: 'deadline_reached',
    expectedSubsequentEditCount: 2,
    expectedUndoStepsToRevert: 3,
    explicitConfirmation: true,
  })
  assert.deepEqual(
    normalizeRevertPostApplyProofFailureRequestV1(request),
    request,
  )

  const calls: unknown[] = []
  const client = createPostApplyProofSchedulerClientV1(
    async (command, args) => {
      calls.push({ command, args })
      return 14
    },
  )
  assert.equal(await client.revert(request!), 14)
  assert.deepEqual(calls, [{
    command: 'revert_post_apply_proof_failure_v1',
    args: { request },
  }])
})

test('atomic proof revert rejects stale, non-revertible, and inconsistent reports', () => {
  const terminal = normalizePostApplyProofProgressV1(progress({
    status: 'blocked',
  }))!
  const failure = terminal.proofFailure!
  assert.equal(
    createRevertPostApplyProofFailureRequestV1(
      terminal,
      9,
      { ...failure, subsequentEditCount: 3 },
    ),
    null,
  )
  for (const invalid of [
    {
      ...createRevertPostApplyProofFailureRequestV1(terminal, 9, failure),
      explicitConfirmation: false,
    },
    {
      ...createRevertPostApplyProofFailureRequestV1(terminal, 9, failure),
      expectedLocation: 'unapplied_redo',
      expectedUndoStepsToRevert: null,
    },
    {
      ...createRevertPostApplyProofFailureRequestV1(terminal, 9, failure),
      expectedReason: 'resource_limit',
    },
    {
      ...createRevertPostApplyProofFailureRequestV1(terminal, 9, failure),
      expectedUndoStepsToRevert: 4,
    },
    {
      ...createRevertPostApplyProofFailureRequestV1(terminal, 9, failure),
      expectedRevision: -0,
    },
  ]) {
    assert.equal(normalizeRevertPostApplyProofFailureRequestV1(invalid), null)
  }
})

test('post-Apply proof v1 binds a canonical job token to exact project authority', () => {
  assert.deepEqual(normalizeStartPostApplyProofJobRequestV1(binding), binding)
  assert.deepEqual(normalizePostApplyProofJobRequestV1({
    ...binding,
    jobToken,
  }), {
    ...binding,
    jobToken,
  })

  for (const invalid of [
    { ...binding, version: 2 },
    { ...binding, projectInstanceId: projectInstanceId.toUpperCase() },
    { ...binding, projectId: '00000000-0000-0000-0000-000000000000' },
    { ...binding, revision: -0 },
    { ...binding, revision: -1 },
    { ...binding, revision: Number.MAX_SAFE_INTEGER + 1 },
  ]) {
    assert.equal(normalizeStartPostApplyProofJobRequestV1(invalid), null)
  }
  for (const invalidToken of [
    'not-a-uuid',
    jobToken.toUpperCase(),
    '00000000-0000-0000-0000-000000000000',
  ]) {
    assert.equal(normalizePostApplyProofJobRequestV1({
      ...binding,
      jobToken: invalidToken,
    }), null)
  }
})

test('post-Apply proof requests are exact own-data snapshots under accessors and proxies', () => {
  const accessor = { ...binding }
  Object.defineProperty(accessor, 'projectId', {
    enumerable: true,
    get() {
      throw new Error('must not be read')
    },
  })
  assert.equal(normalizeStartPostApplyProofJobRequestV1(accessor), null)
  assert.equal(normalizeStartPostApplyProofJobRequestV1({
    ...binding,
    extra: true,
  }), null)

  let getCalls = 0
  const transparent = new Proxy({ ...binding, jobToken }, {
    get() {
      getCalls += 1
      throw new Error('must not be read')
    },
  })
  assert.notEqual(normalizePostApplyProofJobRequestV1(transparent), null)
  assert.equal(getCalls, 0)

  const revoked = Proxy.revocable({ ...binding, jobToken }, {})
  revoked.revoke()
  assert.equal(normalizePostApplyProofJobRequestV1(revoked.proxy), null)
})

test('post-Apply proof progress enforces safe bounded aggregate counts', () => {
  for (const invalid of [
    progress({ provenPairCount: -0 }),
    progress({ provenPairCount: -1 }),
    progress({ provenPairCount: 6 }),
    progress({ provenPairCount: Number.MAX_SAFE_INTEGER + 1 }),
    progress({ totalPairCount: -0 }),
    progress({ totalPairCount: -1 }),
    progress({ totalPairCount: Number.MAX_SAFE_INTEGER + 1 }),
    progress({ status: 'unknown' }),
    progress({ version: 1 + Number.EPSILON }),
    progress({ status: 'certified', provenPairCount: 4 }),
    progress({ totalPairCount: 0 }),
    progress({ status: 'certified', provenPairCount: 0, totalPairCount: 0 }),
  ]) {
    assert.equal(normalizePostApplyProofProgressV1(invalid), null)
  }
  assert.notEqual(normalizePostApplyProofProgressV1(progress({
    totalPairCount: Number.MAX_SAFE_INTEGER,
  })), null)
  assert.notEqual(normalizePostApplyProofProgressV1(progress({
    status: 'certified',
    provenPairCount: Number.MAX_SAFE_INTEGER,
    totalPairCount: Number.MAX_SAFE_INTEGER,
  })), null)
})

test('post-Apply proof v1 rejects partial counts outside certified terminal results', () => {
  for (const status of POST_APPLY_PROOF_STATUSES_V1) {
    if (status === 'certified') continue
    assert.equal(normalizePostApplyProofProgressV1(progress({
      status,
      provenPairCount: 1,
    })), null, status)
  }
  assert.equal(normalizePostApplyProofProgressV1(progress({
    status: 'certified',
    provenPairCount: 4,
  })), null)
})

test('terminal failure reports are exact, status-bound, and fail closed', () => {
  assert.equal(normalizePostApplyProofProgressV1(progress({
    status: 'blocked',
    proofFailure: null,
  })), null)
  assert.equal(normalizePostApplyProofProgressV1(progress({
    status: 'proving',
    proofFailure: failureForStatus('blocked'),
  })), null)
  assert.equal(normalizePostApplyProofProgressV1(progress({
    status: 'unknown_resource_limit',
    proofFailure: failureForStatus('unknown_cancelled'),
  })), null)
  assert.notEqual(normalizePostApplyProofProgressV1(progress({
    status: 'unknown_deadline_reached',
  })), null)
})

test('post-Apply proof progress rejects raw proof details and every non-exact own field', () => {
  for (const forbidden of [
    'pairIds',
    'geometry',
    'path',
    'error',
    'authorizesProjectMutation',
    'revertAvailable',
  ]) {
    assert.equal(normalizePostApplyProofProgressV1({
      ...progress(),
      [forbidden]: forbidden === 'pairIds' ? ['secret-pair'] : true,
    }), null)
  }

  const hidden = progress()
  Object.defineProperty(hidden, 'privatePath', {
    value: 'C:\\private\\proof.json',
  })
  assert.equal(normalizePostApplyProofProgressV1(hidden), null)
  assert.equal(normalizePostApplyProofProgressV1({
    ...progress(),
    [Symbol('private')]: true,
  }), null)
  assert.equal(normalizePostApplyProofProgressV1(Object.assign(
    Object.create(progress()),
    {},
  )), null)
})

test('post-Apply proof parser never invokes getters and fails closed on hostile proxies', () => {
  const accessor = progress()
  Object.defineProperty(accessor, 'status', {
    enumerable: true,
    get() {
      throw new Error('must not be read')
    },
  })
  assert.equal(normalizePostApplyProofProgressV1(accessor), null)

  let getCalls = 0
  const transparent = new Proxy(progress(), {
    get() {
      getCalls += 1
      throw new Error('must not be read')
    },
  })
  assert.notEqual(normalizePostApplyProofProgressV1(transparent), null)
  assert.equal(getCalls, 0)

  const throwing = new Proxy(progress(), {
    ownKeys() {
      throw new Error('hostile ownKeys')
    },
  })
  assert.equal(normalizePostApplyProofProgressV1(throwing), null)

  const target = progress()
  const revoked = Proxy.revocable(target, {})
  revoked.revoke()
  assert.equal(normalizePostApplyProofProgressV1(revoked.proxy), null)
})

test('scheduler client validates requests before invoke and cross-checks every response binding', async () => {
  const calls: Readonly<{
    command: string
    args: Readonly<Record<string, unknown>> | undefined
  }>[] = []
  const client = createPostApplyProofSchedulerClientV1(
    async (command, args) => {
      calls.push({ command, args })
      return progress()
    },
  )

  assert.deepEqual(await client.start(binding), progress())
  assert.deepEqual(await client.poll({ ...binding, jobToken }), progress())
  assert.deepEqual(calls, [
    {
      command: 'start_post_apply_proof_job_v1',
      args: { request: binding },
    },
    {
      command: 'poll_post_apply_proof_job_v1',
      args: { request: { ...binding, jobToken } },
    },
  ])

  await assert.rejects(
    client.start({ ...binding, revision: -0 }),
    (error) =>
      error instanceof PostApplyProofSchedulerClientError
      && error.reason === 'invalid_request',
  )
  assert.equal(calls.length, 2)

  const mismatched = createPostApplyProofSchedulerClientV1(async () =>
    progress({ revision: 9 }))
  await assert.rejects(
    mismatched.start(binding),
    (error) =>
      error instanceof PostApplyProofSchedulerClientError
      && error.reason === 'invalid_response',
  )

  const inconsistent = createPostApplyProofSchedulerClientV1(async () =>
    progress({ status: 'certified', provenPairCount: 4 }))
  await assert.rejects(
    inconsistent.start(binding),
    (error) =>
      error instanceof PostApplyProofSchedulerClientError
      && error.reason === 'invalid_response',
  )
  await assert.rejects(
    inconsistent.poll({ ...binding, jobToken }),
    (error) =>
      error instanceof PostApplyProofSchedulerClientError
      && error.reason === 'invalid_response',
  )
})

test('scheduler client redacts transport failures and cancellation exposes no inferred authority', async () => {
  const secret = 'C:\\private\\native-proof-error.json'
  const failing = createPostApplyProofSchedulerClientV1(async () => {
    throw new Error(secret)
  })
  await assert.rejects(
    failing.poll({ ...binding, jobToken }),
    (error) => {
      assert.equal(error instanceof PostApplyProofSchedulerClientError, true)
      assert.equal(
        (error as PostApplyProofSchedulerClientError).reason,
        'transport_failure',
      )
      assert.equal(String(error).includes(secret), false)
      return true
    },
  )

  const calls: unknown[] = []
  const client = createPostApplyProofSchedulerClientV1(
    async (command, args) => {
      calls.push({ command, args })
      return {
        authorizesProjectMutation: true,
        revertAvailable: true,
      }
    },
  )
  assert.equal(await client.cancel({ ...binding, jobToken }), undefined)
  assert.deepEqual(calls, [{
    command: 'cancel_post_apply_proof_job_v1',
    args: { request: { ...binding, jobToken } },
  }])

  await assert.rejects(
    failing.cancel({ ...binding, jobToken }),
    (error) => {
      assert.equal(error instanceof PostApplyProofSchedulerClientError, true)
      assert.equal(
        (error as PostApplyProofSchedulerClientError).reason,
        'transport_failure',
      )
      assert.equal(String(error).includes(secret), false)
      return true
    },
  )
})
