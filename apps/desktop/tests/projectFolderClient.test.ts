import assert from 'node:assert/strict'
import test from 'node:test'

import {
  createProjectFolderClient,
  normalizeProjectFolderResponse,
  ProjectFolderClientError,
  projectFolderClientErrorCode,
  projectFolderClientErrorMessage,
} from '../src/lib/projectFolderClient.ts'

const INSTANCE_ID = '11111111-1111-4111-8111-111111111111'
const PROJECT_ID = '22222222-2222-4222-8222-222222222222'

test('invokes only the two pathless native commands with a strict locale', async () => {
  const calls: Array<readonly [string, unknown]> = []
  const client = createProjectFolderClient(async (command, args) => {
    calls.push([command, args])
    return { canceled: false, project: validSnapshot() }
  }, () => true)

  await client.open('ja')
  await client.saveAsNew('en')
  assert.deepEqual(calls, [
    ['open_project_folder', { locale: 'ja' }],
    ['save_project_folder_as', { locale: 'en' }],
  ])
  assert.doesNotMatch(JSON.stringify(calls), /path|bytes|targetName/u)
})

test('strictly admits an exact pathless response and rejects response drift', () => {
  const response = normalizeProjectFolderResponse({
    canceled: false,
    project: validSnapshot(),
  })
  assert.equal(response.project.current_path, null)
  assert.equal(Object.isFrozen(response), true)
  assert.equal(Object.isFrozen(response.project), true)

  for (const invalid of [
    { canceled: false, project: validSnapshot(), path: 'C:\\secret' },
    { canceled: 'false', project: validSnapshot() },
    { canceled: false, project: validSnapshot({ current_path: 'C:\\secret' }) },
    { canceled: false, project: validSnapshot({ revision: -0 }) },
    { canceled: false, project: validSnapshot({ private_bytes: [1, 2, 3] }) },
  ]) {
    assert.throws(
      () => normalizeProjectFolderResponse(invalid),
      invalidResponse,
    )
  }
})

test('does not invoke hostile response getters', () => {
  let getterCalls = 0
  const hostile = {
    canceled: false,
  } as Record<string, unknown>
  Object.defineProperty(hostile, 'project', {
    enumerable: true,
    get() {
      getterCalls += 1
      return validSnapshot()
    },
  })
  assert.throws(
    () => normalizeProjectFolderResponse(hostile),
    invalidResponse,
  )
  assert.equal(getterCalls, 0)
})

test('maps only fixed native categories and redacts arbitrary failures', async () => {
  const stale = createProjectFolderClient(async () => {
    throw 'project_folder_project_changed'
  }, () => true)
  await assert.rejects(stale.saveAsNew('ja'), (error: unknown) =>
    error instanceof ProjectFolderClientError
      && error.code === 'project_changed')

  const recovery = createProjectFolderClient(async () => {
    throw 'project_folder_recovery_required'
  }, () => true)
  await assert.rejects(recovery.open('en'), (error: unknown) =>
    error instanceof ProjectFolderClientError
      && error.code === 'recovery_required')

  const unsupportedReplacement = createProjectFolderClient(async () => {
    throw 'project_folder_replacement_unsupported'
  }, () => true)
  await assert.rejects(unsupportedReplacement.saveAsNew('ja'), (error: unknown) =>
    error instanceof ProjectFolderClientError
      && error.code === 'replacement_unsupported')

  const hostile = createProjectFolderClient(async () => {
    throw new Error('C:\\Users\\alice\\private-project\\manifest.json')
  }, () => true)
  await assert.rejects(hostile.open('ja'), (error: unknown) => {
    assert.equal(projectFolderClientErrorCode(error), 'invalid_response')
    assert.ok(error instanceof ProjectFolderClientError)
    assert.doesNotMatch(error.message, /alice|manifest|private/u)
    assert.equal('cause' in error, false)
    return true
  })
})

test('formats actionable Japanese and English messages without raw payloads', () => {
  const expectations = [
    ['target_exists', '別のプロジェクト', 'another project'],
    ['too_large', 'サイズ上限', 'size limit'],
    ['link_or_special_entry', '特殊ファイル', 'special file'],
    ['project_changed', 'プロジェクトが変更', 'project changed'],
    ['recovery_required', '外付けドライブ', 'external drive'],
    ['replacement_unsupported', '新しいフォルダー名', 'new folder name'],
    ['busy', '処理中', 'is running'],
  ] as const
  for (const [code, japanese, english] of expectations) {
    const error = new ProjectFolderClientError(code)
    assert.match(projectFolderClientErrorMessage(error, 'ja'), new RegExp(japanese, 'u'))
    assert.match(projectFolderClientErrorMessage(error, 'en'), new RegExp(english, 'iu'))
  }
  assert.doesNotMatch(
    projectFolderClientErrorMessage(
      new Error(String.raw`C:\Users\alice\secret`),
      'ja',
    ),
    /alice|secret|Users/u,
  )
})

function invalidResponse(error: unknown) {
  return error instanceof ProjectFolderClientError
    && error.code === 'invalid_response'
}

function validSnapshot(overrides: Record<string, unknown> = {}) {
  return {
    project_instance_id: INSTANCE_ID,
    project_id: PROJECT_ID,
    name: 'Expanded project',
    memo: '',
    current_path: null,
    revision: 4,
    saved_revision: 4,
    is_dirty: false,
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
    element_metadata: {
      vertices: [],
      edges: [],
      faces: [],
    },
    annotations: { schema_version: 1, annotations: [] },
    underlays: { schema_version: 1, underlays: [] },
    fold_model_fingerprint: 'a'.repeat(64),
    can_undo: false,
    can_redo: false,
    cutting_allowed: false,
    ...overrides,
  }
}
