import assert from 'node:assert/strict'
import test from 'node:test'

import {
  createGlobalFlatFoldabilityNativeTransport,
  GlobalFlatFoldabilityNativeError,
} from '../src/lib/globalFlatFoldabilityNative.ts'

const PROJECT_ID = '00000000-0000-4000-8000-000000000001'
const JOB_ID = '00000000-0000-4000-8000-000000000002'
const FOLD_MODEL_FINGERPRINT = 'a'.repeat(64)
const CONTEXT = {
  projectInstanceId: '018f47d1-5ca0-75b1-a53a-c579f39f9660',
  projectId: PROJECT_ID,
  revision: 7,
  foldModelFingerprint: FOLD_MODEL_FINGERPRINT,
}
const COUNTS = {
  face_count: 2,
  overlap_cell_count: 1,
  constraint_count: 3,
  search_node_count: 0,
}
const QUEUED = {
  state: 'queued',
  cancel_requested: false,
  progress: {
    model_id: 'convex_faces_facewise_v1',
    phase: 'capturing',
    completed_work: 0,
    total_work: null,
    elapsed_ms: 0,
    counts: {
      face_count: 0,
      overlap_cell_count: 0,
      constraint_count: 0,
      search_node_count: 0,
    },
  },
}
const RUNNING = {
  state: 'running',
  cancel_requested: false,
  progress: {
    model_id: 'convex_faces_facewise_v1',
    phase: 'building_constraints',
    completed_work: 3,
    total_work: null,
    elapsed_ms: 12,
    counts: COUNTS,
  },
}
const COMPLETED = {
  state: 'completed',
  result: {
    verdict: 'unknown',
    summary: {
      model_id: 'convex_faces_facewise_v1',
      elapsed_ms: 15,
      counts: COUNTS,
    },
    reason: 'proof_not_completed',
  },
}
const SOURCE_LIMIT_COMPLETED = {
  ...COMPLETED,
  result: {
    ...COMPLETED.result,
    reason: 'work_limit_reached',
  },
}

test('begin uses the exact camel-case command contract and validates its envelope', async () => {
  const calls: Array<readonly [string, Readonly<Record<string, unknown>> | undefined]> = []
  const transport = createGlobalFlatFoldabilityNativeTransport((command, arguments_) => {
    calls.push([command, arguments_])
    return { job_id: JOB_ID, job: QUEUED }
  })

  const result = await transport.begin(CONTEXT, 30_000)

  assert.equal(result.jobId, JOB_ID)
  assert.deepEqual(result.job, QUEUED)
  assert.deepEqual(calls, [[
    'begin_global_flat_foldability',
    {
      expectedProjectInstanceId: CONTEXT.projectInstanceId,
      expectedProjectId: PROJECT_ID,
      expectedRevision: 7,
      expectedFoldModelFingerprint: FOLD_MODEL_FINGERPRINT,
      timeLimitMs: 30_000,
    },
  ]])
})

test('begin accepts only the bounded immediate source-limit terminal', async () => {
  const transport = createGlobalFlatFoldabilityNativeTransport(() => ({
    job_id: JOB_ID,
    job: SOURCE_LIMIT_COMPLETED,
  }))

  const result = await transport.begin(CONTEXT, 30_000)
  assert.equal(result.jobId, JOB_ID)
  assert.deepEqual(result.job, SOURCE_LIMIT_COMPLETED)
})

test('UUID version and variant bits do not narrow the native Rust contract', async () => {
  const projectId = '00000000-0000-0000-7000-000000000001'
  const jobId = 'ffffffff-ffff-ffff-ffff-ffffffffffff'
  const calls: Array<readonly [string, Readonly<Record<string, unknown>> | undefined]> = []
  const transport = createGlobalFlatFoldabilityNativeTransport((command, arguments_) => {
    calls.push([command, arguments_])
    return { job_id: jobId, job: QUEUED }
  })

  const result = await transport.begin({
    ...CONTEXT,
    projectId,
  }, 30_000)
  assert.equal(result.jobId, jobId)
  assert.equal(calls[0]?.[1]?.expectedProjectId, projectId)
})

test('poll claims a terminal result without requesting progress', async () => {
  const commands: string[] = []
  const transport = createGlobalFlatFoldabilityNativeTransport((command) => {
    commands.push(command)
    return COMPLETED
  })

  assert.deepEqual(await transport.poll(JOB_ID), COMPLETED)
  assert.deepEqual(commands, ['get_global_flat_foldability_result'])
})

test('poll requests progress only for the closed result_unavailable category', async () => {
  const commands: string[] = []
  const transport = createGlobalFlatFoldabilityNativeTransport((command) => {
    commands.push(command)
    if (command === 'get_global_flat_foldability_result') {
      throw {
        category: 'result_unavailable',
        message_ja: 'この文字列を画面へ反映しない',
      }
    }
    return RUNNING
  })

  assert.deepEqual(await transport.poll(JOB_ID), RUNNING)
  assert.deepEqual(commands, [
    'get_global_flat_foldability_result',
    'get_global_flat_foldability_progress',
  ])
})

test('poll accepts a terminal result completed between result and progress requests', async () => {
  const commands: string[] = []
  const transport = createGlobalFlatFoldabilityNativeTransport((command) => {
    commands.push(command)
    if (command === 'get_global_flat_foldability_result') {
      throw { category: 'result_unavailable' }
    }
    return COMPLETED
  })

  assert.deepEqual(await transport.poll(JOB_ID), COMPLETED)
  assert.deepEqual(commands, [
    'get_global_flat_foldability_result',
    'get_global_flat_foldability_progress',
  ])
})

test('another native failure never falls through to progress or reflects raw data', async () => {
  const privateMessage = 'C:\\Users\\alice\\秘密の作品.ori'
  const commands: string[] = []
  const transport = createGlobalFlatFoldabilityNativeTransport((command) => {
    commands.push(command)
    throw { category: 'internal_failure', message_ja: privateMessage }
  })

  await assert.rejects(
    transport.poll(JOB_ID),
    (error: unknown) => {
      assert.ok(error instanceof GlobalFlatFoldabilityNativeError)
      assert.equal(error.category, 'internal_failure')
      assert.equal(String(error).includes(privateMessage), false)
      return true
    },
  )
  assert.deepEqual(commands, ['get_global_flat_foldability_result'])
})

test('malformed begin and job DTO values fail closed', async () => {
  const rejected = [
    { job_id: JOB_ID, job: { ...QUEUED, raw_error: 'private' } },
    { job_id: JOB_ID, job: RUNNING },
    { job_id: JOB_ID, job: COMPLETED },
    { job_id: 'not-an-id', job: QUEUED },
    { job_id: JOB_ID, job: QUEUED, internal_id: 'private' },
  ]
  for (const value of rejected) {
    const transport = createGlobalFlatFoldabilityNativeTransport(() => value)
    await assert.rejects(
      transport.begin(CONTEXT, 5_000),
      GlobalFlatFoldabilityNativeError,
    )
  }
})

test('invalid local requests are rejected before invoking native code', async () => {
  let calls = 0
  const transport = createGlobalFlatFoldabilityNativeTransport(() => {
    calls += 1
    return { job_id: JOB_ID, job: QUEUED }
  })
  const invalidContexts = [
    {
      projectId: 'not-an-id',
      revision: 0,
      foldModelFingerprint: FOLD_MODEL_FINGERPRINT,
    },
    {
      projectId: '00000000-0000-0000-0000-000000000000',
      revision: 0,
      foldModelFingerprint: FOLD_MODEL_FINGERPRINT,
    },
    {
      projectId: PROJECT_ID,
      revision: -1,
      foldModelFingerprint: FOLD_MODEL_FINGERPRINT,
    },
    {
      projectId: PROJECT_ID,
      revision: Number.MAX_SAFE_INTEGER + 1,
      foldModelFingerprint: FOLD_MODEL_FINGERPRINT,
    },
    {
      projectId: PROJECT_ID,
      revision: 0,
      foldModelFingerprint: 'A'.repeat(64),
    },
    {
      projectId: PROJECT_ID,
      revision: 0,
      foldModelFingerprint: 'a'.repeat(63),
    },
  ]
  for (const context of invalidContexts) {
    await assert.rejects(
      transport.begin(context, 30_000),
      GlobalFlatFoldabilityNativeError,
    )
  }
  for (const timeLimit of [999, 300_001, 1.5, Number.NaN]) {
    await assert.rejects(
      transport.begin(CONTEXT, timeLimit),
      GlobalFlatFoldabilityNativeError,
    )
  }
  await assert.rejects(
    transport.poll('not-an-id'),
    GlobalFlatFoldabilityNativeError,
  )
  await assert.rejects(
    transport.cancel('not-an-id'),
    GlobalFlatFoldabilityNativeError,
  )
  assert.equal(calls, 0)
})

test('cancel sends only the opaque job ID and supports repeated calls', async () => {
  const calls: Array<readonly [string, Readonly<Record<string, unknown>> | undefined]> = []
  const transport = createGlobalFlatFoldabilityNativeTransport((command, arguments_) => {
    calls.push([command, arguments_])
  })

  await transport.cancel(JOB_ID)
  await transport.cancel(JOB_ID)

  assert.deepEqual(calls, [
    ['cancel_global_flat_foldability', { jobId: JOB_ID }],
    ['cancel_global_flat_foldability', { jobId: JOB_ID }],
  ])
})

test('hostile response objects and synchronous throws are contained', async () => {
  const accessor = Object.create(null) as Record<string, unknown>
  Object.defineProperty(accessor, 'job_id', {
    enumerable: true,
    get() {
      throw new Error('private getter')
    },
  })
  const hostileValues = [
    accessor,
    new Proxy({}, {
      ownKeys() {
        throw new Error('private proxy')
      },
    }),
  ]
  for (const value of hostileValues) {
    const transport = createGlobalFlatFoldabilityNativeTransport(() => value)
    await assert.rejects(
      transport.begin(CONTEXT, 30_000),
      GlobalFlatFoldabilityNativeError,
    )
  }

  const throwing = createGlobalFlatFoldabilityNativeTransport(() => {
    throw new Error('C:\\private\\source.ori')
  })
  await assert.rejects(
    throwing.begin(CONTEXT, 30_000),
    (error: unknown) => (
      error instanceof GlobalFlatFoldabilityNativeError
      && !String(error).includes('source.ori')
    ),
  )
})
