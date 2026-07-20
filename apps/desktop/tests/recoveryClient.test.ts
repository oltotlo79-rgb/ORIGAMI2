import assert from 'node:assert/strict'
import test from 'node:test'

import {
  createRecoveryClient,
  createWindowCloseHandshake,
  createWindowCloseHandshakeState,
  parseRecoveryCandidate,
  parsePreparedWindowCloseResponse,
  parseRestoredRecoverySnapshot,
  RecoveryClientError,
  WINDOW_CLOSE_STATUS,
  type RecoveryCandidateAvailable,
  type RecoveryExpectedProjectBinding,
  type PreparedWindowCloseResponse,
  type WindowCloseAuthorization,
  type WindowCloseProjectState,
} from '../src/lib/recoveryClient.ts'

const RECOVERY_ID = '1aaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1'
const RECOVERED_PROJECT_ID = '2bbbbbbb-bbbb-4bbb-9bbb-bbbbbbbbbbb2'
const CURRENT_INSTANCE_ID = '3ccccccc-cccc-4ccc-accc-ccccccccccc3'
const CURRENT_PROJECT_ID = '4ddddddd-dddd-4ddd-bddd-ddddddddddd4'
const RESTORED_INSTANCE_ID = '5eeeeeee-eeee-4eee-8eee-eeeeeeeeeee5'
const CLOSE_PREPARE_ID = '6fffffff-ffff-4fff-8fff-fffffffffff6'

const AVAILABLE: RecoveryCandidateAvailable = {
  schema_version: 1,
  status: 'available',
  recovery_id: RECOVERY_ID,
  project_id: RECOVERED_PROJECT_ID,
  updated_at_unix_ms: 1_753_000_000_000,
}

const EXPECTED: RecoveryExpectedProjectBinding = {
  project_instance_id: CURRENT_INSTANCE_ID,
  project_id: CURRENT_PROJECT_ID,
  revision: 12,
}

const CLEAN_PROJECT: WindowCloseProjectState = {
  ...EXPECTED,
  is_dirty: false,
}

function prepared(
  authorization: WindowCloseAuthorization,
): PreparedWindowCloseResponse {
  return {
    schema_version: 1,
    status: 'prepared',
    close_prepare_id: CLOSE_PREPARE_ID,
    ...EXPECTED,
    authorization,
  }
}

function canceled(preparedResponse: PreparedWindowCloseResponse) {
  return {
    ...preparedResponse,
    status: 'canceled' as const,
  }
}

test('admits only the three exact recovery candidate V1 variants', () => {
  assert.deepEqual(parseRecoveryCandidate({
    schema_version: 1,
    status: 'none',
  }), {
    schema_version: 1,
    status: 'none',
  })
  assert.deepEqual(parseRecoveryCandidate({
    schema_version: 1,
    status: 'invalid',
    recovery_id: RECOVERY_ID,
  }), {
    schema_version: 1,
    status: 'invalid',
    recovery_id: RECOVERY_ID,
  })
  assert.deepEqual(parseRecoveryCandidate(AVAILABLE), AVAILABLE)
  assert.deepEqual(parseRecoveryCandidate({
    ...AVAILABLE,
    updated_at_unix_ms: null,
  }), {
    ...AVAILABLE,
    updated_at_unix_ms: null,
  })

  for (const invalid of [
    { schema_version: 2, status: 'none' },
    { schema_version: 1, status: 'future' },
    { schema_version: 1, status: 'none', recovery_id: RECOVERY_ID },
    { ...AVAILABLE, project_name: 'secret document' },
    { ...AVAILABLE, path: 'C:\\private\\work.ori2' },
    { ...AVAILABLE, document: {} },
    { ...AVAILABLE, recovery_id: RECOVERY_ID.toUpperCase() },
    { ...AVAILABLE, project_id: '00000000-0000-0000-0000-000000000000' },
    { ...AVAILABLE, updated_at_unix_ms: -1 },
    { ...AVAILABLE, updated_at_unix_ms: -0 },
    { ...AVAILABLE, updated_at_unix_ms: 1.5 },
    { ...AVAILABLE, updated_at_unix_ms: Number.MAX_SAFE_INTEGER + 1 },
    null,
    [],
  ]) {
    assert.equal(parseRecoveryCandidate(invalid), null)
  }
})

test('candidate admission rejects accessors, symbols, and hostile proxies without reading values', () => {
  let getterCalls = 0
  const accessor = {
    schema_version: 1,
    recovery_id: RECOVERY_ID,
  }
  Object.defineProperty(accessor, 'status', {
    enumerable: true,
    get() {
      getterCalls += 1
      return 'invalid'
    },
  })
  assert.equal(parseRecoveryCandidate(accessor), null)
  assert.equal(getterCalls, 0)

  const symbolCandidate = { ...AVAILABLE }
  Object.defineProperty(symbolCandidate, Symbol('private'), {
    enumerable: true,
    value: 'hidden',
  })
  assert.equal(parseRecoveryCandidate(symbolCandidate), null)

  assert.equal(parseRecoveryCandidate(new Proxy({}, {
    getPrototypeOf() {
      throw new Error('private proxy detail')
    },
  })), null)
  const revocable = Proxy.revocable({ ...AVAILABLE }, {})
  revocable.revoke()
  assert.equal(parseRecoveryCandidate(revocable.proxy), null)
})

test('get candidate invokes the exact discovery command and redacts native failures', async () => {
  const calls: Array<readonly [string, unknown]> = []
  const client = createRecoveryClient(async (command, args) => {
    calls.push([command, args])
    return { schema_version: 1, status: 'none' }
  })
  assert.deepEqual(await client.getCandidate(), {
    schema_version: 1,
    status: 'none',
  })
  assert.deepEqual(calls, [['get_recovery_candidate', undefined]])

  const failing = createRecoveryClient(async () => {
    throw new Error('C:\\private\\recovery\\slot.json')
  })
  await assert.rejects(failing.getCandidate(), (error: unknown) => {
    assert.ok(error instanceof RecoveryClientError)
    assert.equal(error.code, 'native_unavailable')
    assert.doesNotMatch(error.message, /private|slot\.json/u)
    assert.equal('cause' in error, false)
    return true
  })
})

test('restore sends the exact bound request and admits only a fresh dirty pathless snapshot', async () => {
  const calls: Array<readonly [string, unknown]> = []
  const nativeSnapshot = validSnapshot()
  const client = createRecoveryClient(async (command, args) => {
    calls.push([command, args])
    return nativeSnapshot
  })

  const restored = await client.restore(AVAILABLE, EXPECTED)
  assert.deepEqual(calls, [[
    'restore_recovery',
    {
      request: {
        schema_version: 1,
        recovery_id: RECOVERY_ID,
        expected_project_id: CURRENT_PROJECT_ID,
        expected_instance_id: CURRENT_INSTANCE_ID,
        expected_revision: 12,
      },
    },
  ]])
  assert.deepEqual(restored, nativeSnapshot)
  assert.notEqual(restored, nativeSnapshot)
  assert.notEqual(restored.paper, nativeSnapshot.paper)
  assert.equal(restored.current_path, null)
  assert.equal(restored.is_dirty, true)
  assert.equal(restored.revision, 0)
  assert.equal(restored.can_undo, false)
  assert.equal(restored.can_redo, false)
})

test('restore rejects request drift before invoke', async () => {
  let calls = 0
  const client = createRecoveryClient(async () => {
    calls += 1
    return validSnapshot()
  })

  for (const [candidate, expected] of [
    [{ ...AVAILABLE, path: 'C:\\private.ori2' }, EXPECTED],
    [{ ...AVAILABLE, recovery_id: 'not-a-uuid' }, EXPECTED],
    [AVAILABLE, { ...EXPECTED, revision: -1 }],
    [AVAILABLE, { ...EXPECTED, extra: true }],
    [AVAILABLE, { ...EXPECTED, project_instance_id: CURRENT_INSTANCE_ID.toUpperCase() }],
  ] as const) {
    await assert.rejects(
      client.restore(
        candidate as RecoveryCandidateAvailable,
        expected as RecoveryExpectedProjectBinding,
      ),
      (error: unknown) =>
        error instanceof RecoveryClientError
        && error.code === 'invalid_request',
    )
  }
  assert.equal(calls, 0)
})

test('restore rejects identity, fresh-editor, envelope, and nested DTO drift', () => {
  for (const invalid of [
    validSnapshot({ project_instance_id: CURRENT_INSTANCE_ID }),
    validSnapshot({ project_id: CURRENT_PROJECT_ID }),
    validSnapshot({ current_path: 'C:\\private\\original.ori2' }),
    validSnapshot({ is_dirty: false }),
    validSnapshot({ revision: 1 }),
    validSnapshot({ saved_revision: 0 }),
    validSnapshot({ can_undo: true }),
    validSnapshot({ can_redo: true }),
    { ...validSnapshot(), unknown: true },
    validSnapshot({ paper: { ...validSnapshot().paper, private_path: 'secret' } }),
    validSnapshot({
      crease_pattern: {
        vertices: [{ id: RECOVERY_ID, position: { x: Number.NaN, y: 0 } }],
        edges: [],
      },
    }),
    validSnapshot({ numeric_expressions: { future: true } }),
    validSnapshot({
      geometric_constraints: {
        schema_version: 1,
        constraints: [],
        future: true,
      },
    }),
    validSnapshot({
      project_layers: {
        ...validSnapshot().project_layers,
        future: true,
      },
    }),
    validSnapshot({
      project_layers: {
        schema_version: 1,
        layers: [
          ...validSnapshot().project_layers.layers,
          {
            id: '10000000-0000-4000-8000-000000000001',
            name: 'Details',
            content_kind: 'crease_pattern',
          },
        ],
        edge_assignments: [{
          edge: RECOVERY_ID,
          layer: '10000000-0000-4000-8000-000000000001',
        }],
      },
    }),
  ]) {
    assert.equal(
      parseRestoredRecoverySnapshot(invalid, AVAILABLE, EXPECTED),
      null,
    )
  }
})

test('restore preserves declarative instruction text but rejects executable pose data', () => {
  const declarativeStep = {
    id: RECOVERY_ID,
    title: '中割り折り（説明）',
    description: '説明テンプレート',
    caution: '物理操作は自動実行しません。',
    duration_ms: 1_500,
    visual: {
      camera: null,
      arrows: [],
      focus_points: [],
      hand_guides: [],
    },
    pose: {
      model: 'declarative_only_v1',
      source_model_fingerprint: 'f'.repeat(64),
      fixed_face: null,
      hinge_angles: [],
    },
  }
  const restored = parseRestoredRecoverySnapshot(
    validSnapshot({
      instruction_timeline: { steps: [declarativeStep] },
    }),
    AVAILABLE,
    EXPECTED,
  )
  assert.ok(restored)
  assert.deepEqual(restored.instruction_timeline.steps, [declarativeStep])

  for (const pose of [
    { ...declarativeStep.pose, fixed_face: CURRENT_PROJECT_ID },
    {
      ...declarativeStep.pose,
      hinge_angles: [{
        edge: CURRENT_PROJECT_ID,
        angle_degrees: 45,
      }],
    },
  ]) {
    assert.equal(
      parseRestoredRecoverySnapshot(
        validSnapshot({
          instruction_timeline: {
            steps: [{ ...declarativeStep, pose }],
          },
        }),
        AVAILABLE,
        EXPECTED,
      ),
      null,
    )
  }
})

test('restore response admission rejects accessors and proxies without leaking details', async () => {
  let getterCalls = 0
  const accessor = validSnapshot()
  Object.defineProperty(accessor, 'current_path', {
    enumerable: true,
    get() {
      getterCalls += 1
      return null
    },
  })
  assert.equal(
    parseRestoredRecoverySnapshot(accessor, AVAILABLE, EXPECTED),
    null,
  )
  assert.equal(getterCalls, 0)

  const nestedProxy = validSnapshot({
    paper: new Proxy({}, {
      ownKeys() {
        throw new Error('C:\\private\\paper.json')
      },
    }),
  })
  assert.equal(
    parseRestoredRecoverySnapshot(nestedProxy, AVAILABLE, EXPECTED),
    null,
  )

  const client = createRecoveryClient(async () => nestedProxy)
  await assert.rejects(
    client.restore(AVAILABLE, EXPECTED),
    (error: unknown) => {
      assert.ok(error instanceof RecoveryClientError)
      assert.equal(error.code, 'invalid_response')
      assert.doesNotMatch(error.message, /private|paper\.json/u)
      return true
    },
  )
})

test('discard supports available and invalid candidates with an exact V1 acknowledgement', async () => {
  const calls: Array<readonly [string, unknown]> = []
  const client = createRecoveryClient(async (command, args) => {
    calls.push([command, args])
    return { schema_version: 1, status: 'discarded' }
  })
  const invalid = {
    schema_version: 1,
    status: 'invalid',
    recovery_id: RECOVERY_ID,
  } as const

  assert.deepEqual(await client.discard(AVAILABLE), {
    schema_version: 1,
    status: 'discarded',
  })
  assert.deepEqual(await client.discard(invalid), {
    schema_version: 1,
    status: 'discarded',
  })
  assert.deepEqual(calls, [
    [
      'discard_recovery',
      { request: { schema_version: 1, recovery_id: RECOVERY_ID } },
    ],
    [
      'discard_recovery',
      { request: { schema_version: 1, recovery_id: RECOVERY_ID } },
    ],
  ])

  for (const response of [
    { schema_version: 2, status: 'discarded' },
    { schema_version: 1, status: 'discarded', path: 'private' },
    { schema_version: 1, status: 'future' },
  ]) {
    const malformed = createRecoveryClient(async () => response)
    await assert.rejects(
      malformed.discard(invalid),
      (error: unknown) =>
        error instanceof RecoveryClientError
        && error.code === 'invalid_response',
    )
  }
})

test('window close preparation sends and admits only the exact bound V1 handshake', async () => {
  const calls: Array<readonly [string, unknown]> = []
  const client = createRecoveryClient(async (command, args) => {
    calls.push([command, args])
    const request = (args as { request: RecoveryExpectedProjectBinding & {
      authorization: WindowCloseAuthorization
    } }).request
    return {
      schema_version: 1,
      status: 'prepared',
      close_prepare_id: CLOSE_PREPARE_ID,
      project_instance_id: request.project_instance_id,
      project_id: request.project_id,
      revision: request.revision,
      authorization: request.authorization,
    }
  })

  assert.deepEqual(
    await client.prepareWindowClose(EXPECTED, 'clean'),
    prepared('clean'),
  )
  assert.deepEqual(
    await client.prepareWindowClose(EXPECTED, 'discard_confirmed'),
    prepared('discard_confirmed'),
  )
  assert.deepEqual(calls, [
    [
      'prepare_window_close',
      {
        request: {
          schema_version: 1,
          project_instance_id: CURRENT_INSTANCE_ID,
          project_id: CURRENT_PROJECT_ID,
          revision: 12,
          authorization: 'clean',
        },
      },
    ],
    [
      'prepare_window_close',
      {
        request: {
          schema_version: 1,
          project_instance_id: CURRENT_INSTANCE_ID,
          project_id: CURRENT_PROJECT_ID,
          revision: 12,
          authorization: 'discard_confirmed',
        },
      },
    ],
  ])
})

test('window close cancellation sends and requires the exact prepared token echo', async () => {
  const calls: Array<readonly [string, unknown]> = []
  const client = createRecoveryClient(async (command, args) => {
    calls.push([command, args])
    return {
      ...prepared('clean'),
      status: 'canceled',
    }
  })
  assert.deepEqual(await client.cancelWindowClose(prepared('clean')), {
    ...prepared('clean'),
    status: 'canceled',
  })
  assert.deepEqual(calls, [[
    'cancel_window_close_prepare',
    {
      request: {
        schema_version: 1,
        close_prepare_id: CLOSE_PREPARE_ID,
        project_instance_id: CURRENT_INSTANCE_ID,
        project_id: CURRENT_PROJECT_ID,
        revision: 12,
        authorization: 'clean',
      },
    },
  ]])
})

test('window close cancellation rejects hostile tokens and response drift fail-closed', async () => {
  let calls = 0
  const client = createRecoveryClient(async () => {
    calls += 1
    return canceled(prepared('clean'))
  })
  let getterCalls = 0
  const accessor = prepared('clean') as Record<string, unknown>
  Object.defineProperty(accessor, 'close_prepare_id', {
    enumerable: true,
    get() {
      getterCalls += 1
      return CLOSE_PREPARE_ID
    },
  })
  for (const invalid of [
    { ...prepared('clean'), close_prepare_id: CLOSE_PREPARE_ID.toUpperCase() },
    { ...prepared('clean'), unknown: true },
    accessor,
    new Proxy({}, {
      ownKeys() {
        throw new Error('C:\\private\\token.json')
      },
    }),
  ]) {
    await assert.rejects(
      client.cancelWindowClose(
        invalid as PreparedWindowCloseResponse,
      ),
      (error: unknown) =>
        error instanceof RecoveryClientError && error.code === 'invalid_request',
    )
  }
  assert.equal(calls, 0)
  assert.equal(getterCalls, 0)

  for (const response of [
    { ...canceled(prepared('clean')), status: 'future' },
    { ...canceled(prepared('clean')), close_prepare_id: RECOVERY_ID },
    { ...canceled(prepared('clean')), revision: 13 },
    { ...canceled(prepared('clean')), authorization: 'discard_confirmed' },
    { ...canceled(prepared('clean')), private_path: 'C:\\private\\work.ori2' },
  ]) {
    const malformed = createRecoveryClient(async () => response)
    await assert.rejects(
      malformed.cancelWindowClose(prepared('clean')),
      (error: unknown) =>
        error instanceof RecoveryClientError && error.code === 'invalid_response',
    )
  }
})

test('window close DTOs reject request and response drift, accessors, and proxies', async () => {
  let calls = 0
  let requestGetterCalls = 0
  const accessorBinding = { ...EXPECTED }
  Object.defineProperty(accessorBinding, 'revision', {
    enumerable: true,
    get() {
      requestGetterCalls += 1
      return 12
    },
  })
  const neverInvoked = createRecoveryClient(async () => {
    calls += 1
    return prepared('clean')
  })
  for (const [binding, authorization] of [
    [{ ...EXPECTED, project_instance_id: CURRENT_INSTANCE_ID.toUpperCase() }, 'clean'],
    [{ ...EXPECTED, project_id: '00000000-0000-0000-0000-000000000000' }, 'clean'],
    [{ ...EXPECTED, revision: -0 }, 'clean'],
    [{ ...EXPECTED, revision: Number.MAX_SAFE_INTEGER + 1 }, 'clean'],
    [{ ...EXPECTED, private_path: 'C:\\private\\work.ori2' }, 'clean'],
    [accessorBinding, 'clean'],
    [new Proxy({}, {
      getPrototypeOf() {
        throw new Error('C:\\private\\binding.json')
      },
    }), 'clean'],
    [EXPECTED, 'future'],
  ] as const) {
    await assert.rejects(
      neverInvoked.prepareWindowClose(
        binding as RecoveryExpectedProjectBinding,
        authorization as WindowCloseAuthorization,
      ),
      (error: unknown) =>
        error instanceof RecoveryClientError && error.code === 'invalid_request',
    )
  }
  assert.equal(calls, 0)
  assert.equal(requestGetterCalls, 0)

  for (const response of [
    { ...prepared('clean'), schema_version: 2 },
    { ...prepared('clean'), status: 'future' },
    { ...prepared('clean'), project_instance_id: RESTORED_INSTANCE_ID },
    { ...prepared('clean'), project_id: RECOVERED_PROJECT_ID },
    { ...prepared('clean'), revision: 13 },
    { ...prepared('clean'), authorization: 'discard_confirmed' },
    { ...prepared('clean'), close_prepare_id: CLOSE_PREPARE_ID.toUpperCase() },
    { ...prepared('clean'), path: 'C:\\private\\work.ori2' },
  ]) {
    assert.equal(
      parsePreparedWindowCloseResponse(response, EXPECTED, 'clean'),
      null,
    )
  }

  let getterCalls = 0
  const accessor = prepared('clean') as Record<string, unknown>
  Object.defineProperty(accessor, 'revision', {
    enumerable: true,
    get() {
      getterCalls += 1
      return 12
    },
  })
  assert.equal(
    parsePreparedWindowCloseResponse(accessor, EXPECTED, 'clean'),
    null,
  )
  assert.equal(getterCalls, 0)
  assert.equal(
    parsePreparedWindowCloseResponse(new Proxy({}, {
      ownKeys() {
        throw new Error('C:\\private\\close.json')
      },
    }), EXPECTED, 'clean'),
    null,
  )

  const failing = createRecoveryClient(async () => {
    throw new Error('C:\\private\\recovery\\slot.ori2')
  })
  await assert.rejects(
    failing.prepareWindowClose(EXPECTED, 'clean'),
    (error: unknown) => {
      assert.ok(error instanceof RecoveryClientError)
      assert.equal(error.code, 'native_unavailable')
      assert.doesNotMatch(error.message, /private|slot\.ori2/u)
      assert.equal('cause' in error, false)
      return true
    },
  )
})

test('close handshake prevents the first clean event and consumes one prepared second event', async () => {
  let project: unknown = CLEAN_PROJECT
  let prepareCalls = 0
  let closeCalls = 0
  let firstPrevented = 0
  let secondPrevented = 0
  const statuses: string[] = []
  const interactionLocks: boolean[] = []
  const order: string[] = []
  const state = createWindowCloseHandshakeState()
  let handshake: ReturnType<typeof createWindowCloseHandshake>
  handshake = createWindowCloseHandshake(state, {
    getBlocker: () => null,
    getProjectState: () => project,
    confirmDiscard: () => {
      throw new Error('clean projects must not ask')
    },
    prepare: async (expected, authorization) => {
      order.push('prepare')
      prepareCalls += 1
      assert.deepEqual(expected, EXPECTED)
      assert.equal(authorization, 'clean')
      return prepared('clean')
    },
    cancel: async (preparedResponse) => canceled(preparedResponse),
    requestClose: async () => {
      closeCalls += 1
      handshake.handle({
        preventDefault() {
          secondPrevented += 1
        },
      })
    },
    setInteractionLocked: (locked) => interactionLocks.push(locked),
    setStatus: (status) => statuses.push(status),
    reportFailure: () => assert.fail('successful close must not report failure'),
  })

  handshake.handle({
    preventDefault() {
      order.push('prevent')
      firstPrevented += 1
    },
  })
  assert.equal(firstPrevented, 1)
  assert.deepEqual(order, ['prevent', 'prepare'])
  await settlePromises()

  assert.equal(prepareCalls, 1)
  assert.equal(closeCalls, 1)
  assert.equal(secondPrevented, 0)
  assert.equal(state.allow_once, false)
  assert.equal(state.in_flight, false)
  assert.deepEqual(interactionLocks, [true, false])
  assert.deepEqual(statuses, [WINDOW_CLOSE_STATUS.preparing])
  project = null
})

test('dirty cancellation never invokes native and duplicate close stays single-flight', async () => {
  let project: unknown = { ...CLEAN_PROJECT, is_dirty: true }
  let confirmResult = false
  let confirmations = 0
  let prepareCalls = 0
  let closeCalls = 0
  let prevented = 0
  const statuses: string[] = []
  const interactionLocks: boolean[] = []
  const pending = deferred<PreparedWindowCloseResponse>()
  const state = createWindowCloseHandshakeState()
  let handshake: ReturnType<typeof createWindowCloseHandshake>
  handshake = createWindowCloseHandshake(state, {
    getBlocker: () => null,
    getProjectState: () => project,
    confirmDiscard: () => {
      confirmations += 1
      return confirmResult
    },
    prepare: (_expected, authorization) => {
      prepareCalls += 1
      assert.equal(authorization, 'discard_confirmed')
      return pending.promise
    },
    cancel: async (preparedResponse) => canceled(preparedResponse),
    requestClose: async () => {
      closeCalls += 1
      handshake.handle({ preventDefault: () => assert.fail('one-shot was not allowed') })
    },
    setInteractionLocked: (locked) => interactionLocks.push(locked),
    setStatus: (status) => statuses.push(status),
    reportFailure: () => assert.fail('no failure expected'),
  })
  const event = {
    preventDefault() {
      prevented += 1
    },
  }

  handshake.handle(event)
  await settlePromises()
  assert.equal(prevented, 1)
  assert.equal(confirmations, 1)
  assert.equal(prepareCalls, 0)
  assert.equal(statuses.at(-1), WINDOW_CLOSE_STATUS.cancelled)

  confirmResult = true
  handshake.handle(event)
  handshake.handle(event)
  await settlePromises()
  assert.equal(prevented, 3)
  assert.equal(confirmations, 2)
  assert.equal(prepareCalls, 1)
  assert.equal(statuses.at(-1), WINDOW_CLOSE_STATUS.preparing)

  pending.resolve(prepared('discard_confirmed'))
  await settlePromises()
  assert.equal(closeCalls, 1)
  assert.equal(state.in_flight, false)
  assert.deepEqual(interactionLocks, [true, false])
  project = null
})

test('a clean-to-dirty race is stale and never receives the one-shot close', async () => {
  let project: unknown = CLEAN_PROJECT
  let closeCalls = 0
  let failures = 0
  let cancelCalls = 0
  const interactionLocks: boolean[] = []
  const statuses: string[] = []
  const pending = deferred<PreparedWindowCloseResponse>()
  const handshake = createWindowCloseHandshake(
    createWindowCloseHandshakeState(),
    {
      getBlocker: () => null,
      getProjectState: () => project,
      confirmDiscard: () => false,
      prepare: () => pending.promise,
      cancel: async (preparedResponse) => {
        cancelCalls += 1
        return canceled(preparedResponse)
      },
      requestClose: async () => {
        closeCalls += 1
      },
      setInteractionLocked: (locked) => interactionLocks.push(locked),
      setStatus: (status) => statuses.push(status),
      reportFailure: () => {
        failures += 1
      },
    },
  )

  let prevented = 0
  handshake.handle({ preventDefault: () => { prevented += 1 } })
  await settlePromises()
  project = { ...CLEAN_PROJECT, is_dirty: true }
  pending.resolve(prepared('clean'))
  await settlePromises()

  assert.equal(prevented, 1)
  assert.equal(closeCalls, 0)
  assert.equal(cancelCalls, 1)
  assert.equal(failures, 0)
  assert.deepEqual(interactionLocks, [true, false])
  assert.equal(statuses.at(-1), WINDOW_CLOSE_STATUS.stale)
})

test('failed preparation can retry, while disposal invalidates an old StrictMode response', async () => {
  let prepareCalls = 0
  let closeCalls = 0
  let failures = 0
  const statuses: string[] = []
  const interactionLocks: boolean[] = []
  const state = createWindowCloseHandshakeState()
  let active: ReturnType<typeof createWindowCloseHandshake>
  const dependencies = {
    getBlocker: () => null,
    getProjectState: () => CLEAN_PROJECT,
    confirmDiscard: () => false,
    prepare: async () => {
      prepareCalls += 1
      if (prepareCalls === 1) throw new Error('C:\\private\\recovery.ori2')
      return prepared('clean')
    },
    cancel: async (preparedResponse) => canceled(preparedResponse),
    requestClose: async () => {
      closeCalls += 1
      active.handle({ preventDefault: () => assert.fail('one-shot was not allowed') })
    },
    setInteractionLocked: (locked: boolean) => interactionLocks.push(locked),
    setStatus: (status: string) => statuses.push(status),
    reportFailure: () => {
      failures += 1
    },
  }
  active = createWindowCloseHandshake(state, dependencies)

  active.handle({ preventDefault() {} })
  await settlePromises()
  assert.equal(failures, 1)
  assert.equal(statuses.at(-1), WINDOW_CLOSE_STATUS.failed)
  assert.equal(state.in_flight, false)
  assert.deepEqual(interactionLocks, [true, false])

  active.handle({ preventDefault() {} })
  await settlePromises()
  assert.equal(prepareCalls, 2)
  assert.equal(closeCalls, 1)
  assert.deepEqual(interactionLocks, [true, false, true, false])

  const oldPending = deferred<PreparedWindowCloseResponse>()
  const oldCancel = deferred<ReturnType<typeof canceled>>()
  const old = createWindowCloseHandshake(state, {
    ...dependencies,
    prepare: () => oldPending.promise,
    cancel: () => oldCancel.promise,
    requestClose: async () => {
      assert.fail('an unmounted handshake must not close')
    },
  })
  old.handle({ preventDefault() {} })
  await settlePromises()
  old.dispose()
  active = createWindowCloseHandshake(state, dependencies)
  oldPending.resolve(prepared('clean'))
  await settlePromises()
  assert.equal(closeCalls, 1)
  assert.equal(state.in_flight, true)
  assert.equal(state.interaction_locked, true)

  const prepareCallsBeforeDuplicate = prepareCalls
  active.handle({ preventDefault() {} })
  await settlePromises()
  assert.equal(
    prepareCalls,
    prepareCallsBeforeDuplicate,
    'the remounted lifecycle cannot replace an old token before cancellation',
  )
  assert.equal(state.interaction_locked, true)

  oldCancel.resolve(canceled(prepared('clean')))
  await settlePromises()
  assert.equal(state.in_flight, false)
  assert.equal(state.interaction_locked, false)

  active.handle({ preventDefault() {} })
  await settlePromises()
  assert.equal(closeCalls, 2)
})

test('dispatch failure cancels the token, unlocks safely, and permits a retry', async () => {
  let closeCalls = 0
  let cancelCalls = 0
  let failures = 0
  const locks: boolean[] = []
  const statuses: string[] = []
  const state = createWindowCloseHandshakeState()
  let handshake: ReturnType<typeof createWindowCloseHandshake>
  handshake = createWindowCloseHandshake(state, {
    getBlocker: () => null,
    getProjectState: () => CLEAN_PROJECT,
    confirmDiscard: () => false,
    prepare: async () => prepared('clean'),
    cancel: async (preparedResponse) => {
      cancelCalls += 1
      if (cancelCalls === 1) throw new Error('token already expired')
      return canceled(preparedResponse)
    },
    requestClose: async () => {
      closeCalls += 1
      if (closeCalls === 1) throw new Error('window dispatch failed')
      handshake.handle({
        preventDefault: () => assert.fail('the prepared retry must pass once'),
      })
    },
    setInteractionLocked: (locked) => locks.push(locked),
    setStatus: (status) => statuses.push(status),
    reportFailure: () => {
      failures += 1
    },
  })

  handshake.handle({ preventDefault() {} })
  await settlePromises()
  assert.equal(closeCalls, 1)
  assert.equal(cancelCalls, 1)
  assert.equal(state.interaction_locked, false)
  assert.equal(state.in_flight, false)
  assert.equal(statuses.at(-1), WINDOW_CLOSE_STATUS.failed)
  assert.equal(failures, 1, 'dispatch and cancel failures expose one redacted diagnostic')

  handshake.handle({ preventDefault() {} })
  await settlePromises()
  assert.equal(closeCalls, 2)
  assert.equal(cancelCalls, 1)
  assert.deepEqual(locks, [true, false, true, false])

  // Simulate native preventing process exit after the allowed second event.
  // The still-open frontend must be unlocked and able to start a fresh token.
  handshake.handle({ preventDefault() {} })
  await settlePromises()
  assert.equal(closeCalls, 3)
  assert.deepEqual(locks, [true, false, true, false, true, false])
})

test('a resolved close without a second event is canceled and keeps the editor retryable', async () => {
  let cancelCalls = 0
  let failures = 0
  const locks: boolean[] = []
  const statuses: string[] = []
  const state = createWindowCloseHandshakeState()
  const handshake = createWindowCloseHandshake(state, {
    getBlocker: () => null,
    getProjectState: () => CLEAN_PROJECT,
    confirmDiscard: () => false,
    prepare: async () => prepared('clean'),
    cancel: async (preparedResponse) => {
      cancelCalls += 1
      return canceled(preparedResponse)
    },
    requestClose: async () => {},
    setInteractionLocked: (locked) => locks.push(locked),
    setStatus: (status) => statuses.push(status),
    reportFailure: () => {
      failures += 1
    },
  })

  handshake.handle({ preventDefault() {} })
  await settlePromises()
  assert.equal(cancelCalls, 1)
  assert.equal(failures, 1)
  assert.equal(state.allow_once, false)
  assert.equal(state.in_flight, false)
  assert.equal(state.interaction_locked, false)
  assert.deepEqual(locks, [true, false])
  assert.equal(statuses.at(-1), WINDOW_CLOSE_STATUS.failed)
})

test('blocked and hostile close inputs fail closed with fixed redacted statuses', async () => {
  for (const blocker of ['recovery', 'core'] as const) {
    let prevented = 0
    let prepareCalls = 0
    const statuses: string[] = []
    const handshake = createWindowCloseHandshake(
      createWindowCloseHandshakeState(),
      {
        getBlocker: () => blocker,
        getProjectState: () => CLEAN_PROJECT,
        confirmDiscard: () => false,
        prepare: async () => {
          prepareCalls += 1
          return prepared('clean')
        },
        cancel: async (preparedResponse) => canceled(preparedResponse),
        requestClose: async () => assert.fail('blocked close must stay open'),
        setInteractionLocked: () => assert.fail('blocked close must not lock'),
        setStatus: (status) => statuses.push(status),
        reportFailure: () => assert.fail('known blocker is not a failure'),
      },
    )
    handshake.handle({ preventDefault: () => { prevented += 1 } })
    await settlePromises()
    assert.equal(prevented, 1)
    assert.equal(prepareCalls, 0)
    assert.equal(
      statuses.at(-1),
      blocker === 'recovery'
        ? WINDOW_CLOSE_STATUS.recoveryBlocked
        : WINDOW_CLOSE_STATUS.coreBlocked,
    )
  }

  let getterCalls = 0
  const accessor = { ...CLEAN_PROJECT }
  Object.defineProperty(accessor, 'revision', {
    enumerable: true,
    get() {
      getterCalls += 1
      return 12
    },
  })
  const statuses: string[] = []
  let failures = 0
  const handshake = createWindowCloseHandshake(
    createWindowCloseHandshakeState(),
    {
      getBlocker: () => null,
      getProjectState: () => accessor,
      confirmDiscard: () => false,
      prepare: async () => assert.fail('hostile project must not invoke native'),
      cancel: async () => assert.fail('hostile project has no token'),
      requestClose: async () => assert.fail('hostile project must stay open'),
      setInteractionLocked: () => assert.fail('hostile project must not lock'),
      setStatus: (status) => statuses.push(status),
      reportFailure: () => {
        failures += 1
      },
    },
  )
  handshake.handle({ preventDefault() {} })
  await settlePromises()
  assert.equal(getterCalls, 0)
  assert.equal(failures, 1)
  assert.equal(statuses.at(-1), WINDOW_CLOSE_STATUS.failed)
  assert.doesNotMatch(statuses.join(' '), /private|path|accessor/iu)
})

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })
  return { promise, resolve, reject }
}

async function settlePromises() {
  for (let index = 0; index < 8; index += 1) await Promise.resolve()
}

function validSnapshot(overrides: Record<string, unknown> = {}) {
  return {
    project_instance_id: RESTORED_INSTANCE_ID,
    project_id: RECOVERED_PROJECT_ID,
    name: 'Recovered work',
    current_path: null,
    revision: 0,
    saved_revision: null,
    is_dirty: true,
    paper: {
      boundary_vertices: [],
      thickness_mm: 0.1,
      length_display_unit: 'mm',
      cutting_allowed: false,
      front: {
        color: { red: 255, green: 255, blue: 255, alpha: 255 },
        texture_asset: null,
      },
      back: {
        color: { red: 248, green: 248, blue: 245, alpha: 255 },
        texture_asset: null,
      },
    },
    crease_pattern: {
      vertices: [],
      edges: [],
    },
    instruction_timeline: {
      steps: [],
    },
    numeric_expressions: {},
    geometric_constraints: {
      schema_version: 1,
      constraints: [],
    },
    project_layers: {
      schema_version: 1,
      layers: [{
        id: '00000000-0000-4000-8000-000000000001',
        name: 'Crease Pattern',
        content_kind: 'crease_pattern',
        visible: true,
        locked: false,
        opacity: 1,
      }],
      edge_assignments: [],
    },
    fold_model_fingerprint: 'a'.repeat(64),
    can_undo: false,
    can_redo: false,
    cutting_allowed: false,
    ...overrides,
  }
}
