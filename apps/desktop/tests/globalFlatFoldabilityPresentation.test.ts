import assert from 'node:assert/strict'
import test from 'node:test'

import {
  GLOBAL_FLAT_FOLDABILITY_LAYER_ORDER_MODEL_ID,
  GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
  type GlobalFlatFoldabilityErrorCategory,
  type GlobalFlatFoldabilityPhase,
  type GlobalFlatFoldabilityProofCategory,
  type GlobalFlatFoldabilityUnknownReason,
} from '../src/lib/globalFlatFoldability.ts'
import {
  createGlobalFlatFoldabilityPresentation,
  globalFlatFoldabilityErrorMessage,
  globalFlatFoldabilityPhaseLabel,
  globalFlatFoldabilityProofLabel,
  globalFlatFoldabilityUnknownReasonMessage,
} from '../src/lib/globalFlatFoldabilityPresentation.ts'

const COUNTS = {
  face_count: 1_234,
  overlap_cell_count: 23_456,
  constraint_count: 345_678,
  search_node_count: 4_567_890,
}
const SUMMARY = {
  model_id: GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
  elapsed_ms: 65_400,
  counts: COUNTS,
}

test('every monotonic phase has a fixed Japanese label', () => {
  const expected: Readonly<Record<GlobalFlatFoldabilityPhase, string>> = {
    capturing: '編集内容を取得しています',
    validating_local_conditions: '局所平坦折り条件を確認しています',
    building_flat_embedding: '平面配置を構築しています',
    building_overlap_arrangement: '重なり領域を構築しています',
    building_constraints: '層順序の制約を構築しています',
    propagating: '確定した層順序を伝播しています',
    searching: '層順序を探索しています',
    verifying_certificate: '判定根拠を再検証しています',
    completed: '判定結果を確定しています',
  }
  for (const [phase, label] of Object.entries(expected)) {
    assert.equal(
      globalFlatFoldabilityPhaseLabel(phase as GlobalFlatFoldabilityPhase),
      label,
    )
  }
})

test('unknown reasons are converted to a fixed, complete Japanese allowlist', () => {
  const expected: Readonly<Record<GlobalFlatFoldabilityUnknownReason, string>> = {
    unsupported_topology:
      '切断、穴、未接続材料など、初版の対象外となる面構造が含まれています。',
    non_convex_face:
      '凸多角形でない面があるため、初版の対象として判定できませんでした。',
    time_limit_reached:
      '選択した時間制限内に証明を完了できませんでした。時間を延ばして再判定できます。',
    work_limit_reached:
      '処理件数が初版の作業上限に達したため、証明を完了できませんでした。',
    exact_number_limit_reached:
      '正確な数値計算が初版の安全上限に達したため、証明を完了できませんでした。',
    overlap_arrangement_limit_reached:
      '重なり領域の構築が初版の安全上限に達したため、証明を完了できませんでした。',
    constraint_limit_reached:
      '層順序の制約数が初版の安全上限に達したため、証明を完了できませんでした。',
    proof_not_completed:
      '可または不可を確定できる証明を完成できませんでした。',
    local_conditions_indeterminate:
      '局所平坦折り条件に未確定の頂点があるため、全体判定を確定できませんでした。',
  }
  const messages = new Set<string>()
  for (const [reason, message] of Object.entries(expected)) {
    const actual = globalFlatFoldabilityUnknownReasonMessage(
      reason as GlobalFlatFoldabilityUnknownReason,
    )
    assert.equal(actual, message)
    assert.doesNotMatch(actual, /unsupported_|_limit_|proof_|internal/iu)
    messages.add(actual)
  }
  assert.equal(messages.size, Object.keys(expected).length)
})

test('proof and error categories use fixed public wording', () => {
  const proofCategories: readonly GlobalFlatFoldabilityProofCategory[] = [
    'local_conditions_violated',
    'inconsistent_flat_embedding',
    'layer_constraints_contradictory',
    'exhaustive_search_no_solution',
  ]
  for (const category of proofCategories) {
    const label = globalFlatFoldabilityProofLabel(category)
    assert.ok(label.length > 0)
    assert.doesNotMatch(label, /_/u)
  }

  const errorCategories: readonly GlobalFlatFoldabilityErrorCategory[] = [
    'invalid_request',
    'snapshot_unavailable',
    'worker_unavailable',
    'result_unavailable',
    'internal_failure',
  ]
  for (const category of errorCategories) {
    const message = globalFlatFoldabilityErrorMessage(category)
    assert.ok(message.endsWith('。'))
    assert.doesNotMatch(message, /_|panic|stack|path/iu)
  }
})

test('progress reports a phase and count without inventing a percentage', () => {
  const unknownTotal = createGlobalFlatFoldabilityPresentation({
    state: 'running',
    cancel_requested: false,
    progress: {
      model_id: GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
      phase: 'building_overlap_arrangement',
      completed_work: 12_340,
      total_work: null,
      elapsed_ms: 4_500,
      counts: COUNTS,
    },
  })
  assert.equal(unknownTotal.kind, 'running')
  assert.equal(unknownTotal.phaseText, '重なり領域を構築しています')
  assert.equal(unknownTotal.workText, '12,340件完了（総数は計算中）')
  assert.doesNotMatch(unknownTotal.liveText, /12,340件完了/u)
  assert.doesNotMatch(unknownTotal.liveText, /%/u)
  const nextPollInSamePhase = createGlobalFlatFoldabilityPresentation({
    state: 'running',
    cancel_requested: false,
    progress: {
      model_id: GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
      phase: 'building_overlap_arrangement',
      completed_work: 12_341,
      total_work: null,
      elapsed_ms: 4_750,
      counts: COUNTS,
    },
  })
  assert.notEqual(nextPollInSamePhase.workText, unknownTotal.workText)
  assert.equal(nextPollInSamePhase.liveText, unknownTotal.liveText)

  const knownTotal = createGlobalFlatFoldabilityPresentation({
    state: 'running',
    cancel_requested: true,
    progress: {
      model_id: GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
      phase: 'searching',
      completed_work: 250,
      total_work: 1_000,
      elapsed_ms: 65_400,
      counts: COUNTS,
    },
  })
  assert.equal(knownTotal.label, '中止しています')
  assert.equal(knownTotal.workText, '250 / 1,000件完了')
  assert.equal(knownTotal.summaryEntries.at(2)?.value, '1分5秒')
})

test('possible, impossible and unknown presentations retain only public summaries', () => {
  const possible = createGlobalFlatFoldabilityPresentation({
    state: 'completed',
    result: {
      verdict: 'possible',
      summary: SUMMARY,
      layer_order: {
        model_id: GLOBAL_FLAT_FOLDABILITY_LAYER_ORDER_MODEL_ID,
        layer_count: 1_234,
        max_ply: 14,
        reference_face_number: 3,
        layer_view_available: true,
      },
    },
  })
  assert.equal(possible.kind, 'possible')
  assert.equal(possible.label, '可')
  assert.deepEqual(
    possible.resultEntries.map(({ label }) => label),
    ['層順序モデル', '層数', '最大重なり', '基準面', '層順3D表示'],
  )
  assert.ok(possible.resultEntries.some(({ value }) => value === '利用できます'))

  const impossible = createGlobalFlatFoldabilityPresentation({
    state: 'completed',
    result: {
      verdict: 'impossible',
      summary: SUMMARY,
      proof: {
        category: 'inconsistent_flat_embedding',
        face_numbers: [2, 9],
      },
    },
  })
  assert.equal(impossible.kind, 'impossible')
  assert.equal(impossible.label, '不可')
  assert.equal(impossible.resultEntries.at(1)?.value, '面 2、面 9')
  assert.equal(
    impossible.resultEntries.at(1)?.label,
    '対象面（FaceKey順・最大20件）',
  )

  const exhaustive = createGlobalFlatFoldabilityPresentation({
    state: 'completed',
    result: {
      verdict: 'impossible',
      summary: SUMMARY,
      proof: {
        category: 'exhaustive_search_no_solution',
        face_numbers: [1, 2],
      },
    },
  })
  assert.equal(
    exhaustive.resultEntries.at(1)?.value,
    '全体：面 1、面 2（ほか1,232面）',
  )

  const unknown = createGlobalFlatFoldabilityPresentation({
    state: 'completed',
    result: {
      verdict: 'unknown',
      summary: SUMMARY,
      reason: 'time_limit_reached',
    },
  })
  assert.equal(unknown.kind, 'unknown')
  assert.equal(unknown.label, '不明')
  assert.match(unknown.detail, /時間制限/u)
})

test('all six terminal outcomes have distinct text and live announcements', () => {
  const jobs = [
    {
      state: 'completed',
      result: {
        verdict: 'possible',
        summary: SUMMARY,
        layer_order: {
          model_id: GLOBAL_FLAT_FOLDABILITY_LAYER_ORDER_MODEL_ID,
          layer_count: 1_234,
          max_ply: 5,
          reference_face_number: 1,
          layer_view_available: false,
        },
      },
    },
    {
      state: 'completed',
      result: {
        verdict: 'impossible',
        summary: SUMMARY,
        proof: {
          category: 'exhaustive_search_no_solution',
          face_numbers: [1, 2],
        },
      },
    },
    {
      state: 'completed',
      result: {
        verdict: 'unknown',
        summary: SUMMARY,
        reason: 'proof_not_completed',
      },
    },
    { state: 'cancelled', summary: SUMMARY },
    {
      state: 'failed',
      summary: SUMMARY,
      error_category: 'internal_failure',
    },
    { state: 'stale', summary: SUMMARY },
  ]
  const presentations = jobs.map((job) =>
    createGlobalFlatFoldabilityPresentation(job))
  assert.deepEqual(
    presentations.map(({ kind }) => kind),
    ['possible', 'impossible', 'unknown', 'cancelled', 'failed', 'stale'],
  )
  assert.equal(new Set(presentations.map(({ label }) => label)).size, 6)
  assert.equal(new Set(presentations.map(({ icon }) => icon)).size, 6)
  assert.ok(presentations.every(({ liveText }) => liveText.length > 0))
})

test('invalid or hostile DTOs fail closed without reflecting backend data', () => {
  const privateValue = 'C:\\Users\\alice\\秘密の作品.ori at (12.3, 45.6)'
  const invalid = createGlobalFlatFoldabilityPresentation({
    state: 'failed',
    summary: SUMMARY,
    error_category: 'internal_failure',
    raw_error: privateValue,
  })
  assert.equal(invalid.kind, 'failed')
  assert.equal(invalid.label, '計算エラー')
  const visible = JSON.stringify(invalid)
  assert.doesNotMatch(visible, /alice|秘密|12\.3|45\.6/iu)

  const hostile = Object.create(null) as Record<string, unknown>
  Object.defineProperty(hostile, 'state', {
    enumerable: true,
    get() {
      throw new Error(privateValue)
    },
  })
  const hostilePresentation = createGlobalFlatFoldabilityPresentation(hostile)
  assert.equal(hostilePresentation.kind, 'failed')
  assert.doesNotMatch(JSON.stringify(hostilePresentation), /alice|秘密/iu)
})

test('every global flat-foldability helper has complete fixed English wording', () => {
  const phases: Readonly<Record<GlobalFlatFoldabilityPhase, string>> = {
    capturing: 'Capturing edits',
    validating_local_conditions: 'Checking local flat-foldability conditions',
    building_flat_embedding: 'Building the flat embedding',
    building_overlap_arrangement: 'Building overlap regions',
    building_constraints: 'Building layer-order constraints',
    propagating: 'Propagating determined layer order',
    searching: 'Searching layer order',
    verifying_certificate: 'Verifying the result certificate',
    completed: 'Finalizing the result',
  }
  const reasons: Readonly<Record<GlobalFlatFoldabilityUnknownReason, string>> = {
    unsupported_topology:
      'The face structure includes cuts, holes, disconnected material, or another topology outside the initial release scope.',
    non_convex_face:
      'The check is indeterminate because at least one face is not a convex polygon supported by the initial release.',
    time_limit_reached:
      'The proof could not be completed within the selected time limit. Choose a longer limit and run the check again.',
    work_limit_reached:
      'The proof could not be completed because the initial-release work limit was reached.',
    exact_number_limit_reached:
      'The proof could not be completed because exact arithmetic reached the initial-release safety limit.',
    overlap_arrangement_limit_reached:
      'The proof could not be completed because overlap-region construction reached the initial-release safety limit.',
    constraint_limit_reached:
      'The proof could not be completed because the number of layer-order constraints reached the initial-release safety limit.',
    proof_not_completed:
      'A proof establishing Possible or Impossible could not be completed.',
    local_conditions_indeterminate:
      'The global result is indeterminate because at least one vertex has an indeterminate local flat-foldability condition.',
  }
  const proofs: Readonly<Record<GlobalFlatFoldabilityProofCategory, string>> = {
    local_conditions_violated:
      'Explicit violation of local necessary conditions',
    inconsistent_flat_embedding:
      'Path inconsistency in the flat embedding',
    layer_constraints_contradictory:
      'Contradictory layer-order constraints',
    exhaustive_search_no_solution:
      'Exhaustive search completed (no solution)',
  }
  const errors: Readonly<Record<GlobalFlatFoldabilityErrorCategory, string>> = {
    invalid_request:
      'The conditions required to start the check could not be verified. Retry with the current edits.',
    snapshot_unavailable:
      'The edits required for the check could not be captured safely. Retry with the current edits.',
    worker_unavailable:
      'The check could not be started. Wait briefly and retry.',
    result_unavailable:
      'The completed result could not be retrieved safely. Run the check again.',
    internal_failure:
      'The check could not be completed safely. The work was not changed.',
  }

  for (const [phase, expected] of Object.entries(phases)) {
    assert.equal(
      globalFlatFoldabilityPhaseLabel(
        phase as GlobalFlatFoldabilityPhase,
        'en',
      ),
      expected,
    )
  }
  for (const [reason, expected] of Object.entries(reasons)) {
    assert.equal(
      globalFlatFoldabilityUnknownReasonMessage(
        reason as GlobalFlatFoldabilityUnknownReason,
        'en',
      ),
      expected,
    )
  }
  for (const [proof, expected] of Object.entries(proofs)) {
    assert.equal(
      globalFlatFoldabilityProofLabel(
        proof as GlobalFlatFoldabilityProofCategory,
        'en',
      ),
      expected,
    )
  }
  for (const [error, expected] of Object.entries(errors)) {
    assert.equal(
      globalFlatFoldabilityErrorMessage(
        error as GlobalFlatFoldabilityErrorCategory,
        'en',
      ),
      expected,
    )
  }
})

test('English presentations cover idle, progress and every terminal result', () => {
  const idle = createGlobalFlatFoldabilityPresentation(null, 'en')
  assert.equal(idle.kind, 'idle')
  assert.equal(idle.label, 'Not checked')
  assert.equal(idle.detail, 'Select a time limit to check the current edits.')
  assert.deepEqual(
    idle.summaryEntries.map(({ label }) => label),
    ['Check model', 'Target class'],
  )
  assert.equal(
    idle.summaryEntries.at(1)?.value,
    'Convex polygonal faces (no cuts, holes, or disconnected material)',
  )

  const running = createGlobalFlatFoldabilityPresentation({
    state: 'running',
    cancel_requested: false,
    progress: {
      model_id: GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
      phase: 'building_overlap_arrangement',
      completed_work: 12_340,
      total_work: null,
      elapsed_ms: 4_500,
      counts: COUNTS,
    },
  }, 'en')
  assert.equal(running.kind, 'running')
  assert.equal(running.label, 'Checking')
  assert.equal(running.phaseText, 'Building overlap regions')
  assert.equal(
    running.workText,
    '12,340 completed (total still being calculated)',
  )
  assert.equal(running.liveText, 'Checking. Building overlap regions.')
  assert.deepEqual(
    running.summaryEntries.map(({ label }) => label),
    [
      'Check model',
      'Target class',
      'Elapsed time',
      'Faces',
      'Overlap cells',
      'Constraints',
      'Search nodes',
    ],
  )
  assert.deepEqual(
    running.summaryEntries.slice(2).map(({ value }) => value),
    ['4.5 s', '1,234', '23,456', '345,678', '4,567,890'],
  )

  const queued = createGlobalFlatFoldabilityPresentation({
    state: 'queued',
    cancel_requested: false,
    progress: {
      model_id: GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
      phase: 'capturing',
      completed_work: 0,
      total_work: null,
      elapsed_ms: 0,
      counts: COUNTS,
    },
  }, 'en')
  assert.equal(queued.kind, 'queued')
  assert.equal(queued.label, 'Queued')
  assert.equal(queued.phaseText, 'Capturing edits')

  const cancelling = createGlobalFlatFoldabilityPresentation({
    state: 'running',
    cancel_requested: true,
    progress: {
      model_id: GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
      phase: 'searching',
      completed_work: 250,
      total_work: 1_000,
      elapsed_ms: 65_400,
      counts: COUNTS,
    },
  }, 'en')
  assert.equal(cancelling.kind, 'running')
  assert.equal(cancelling.label, 'Cancelling')
  assert.equal(cancelling.workText, '250 / 1,000 completed')
  assert.equal(cancelling.summaryEntries.at(2)?.value, '1 min 5 s')

  const possible = createGlobalFlatFoldabilityPresentation({
    state: 'completed',
    result: {
      verdict: 'possible',
      summary: SUMMARY,
      layer_order: {
        model_id: GLOBAL_FLAT_FOLDABILITY_LAYER_ORDER_MODEL_ID,
        layer_count: 1_234,
        max_ply: 14,
        reference_face_number: 3,
        layer_view_available: true,
      },
    },
  }, 'en')
  assert.equal(possible.label, 'Possible')
  assert.deepEqual(
    possible.resultEntries,
    [
      {
        label: 'Layer-order model',
        value: GLOBAL_FLAT_FOLDABILITY_LAYER_ORDER_MODEL_ID,
      },
      { label: 'Layer count', value: '1,234 layers' },
      { label: 'Maximum overlap', value: '14 ply' },
      { label: 'Reference face', value: 'Face 3' },
      { label: '3D layer-order view', value: 'Available' },
    ],
  )

  const impossible = createGlobalFlatFoldabilityPresentation({
    state: 'completed',
    result: {
      verdict: 'impossible',
      summary: SUMMARY,
      proof: {
        category: 'exhaustive_search_no_solution',
        face_numbers: [1, 2],
      },
    },
  }, 'en')
  assert.equal(impossible.label, 'Impossible')
  assert.deepEqual(
    impossible.resultEntries,
    [
      {
        label: 'Proof type',
        value: 'Exhaustive search completed (no solution)',
      },
      {
        label: 'Target faces (FaceKey order, up to 20)',
        value: 'All: Face 1, Face 2 (1,232 more faces)',
      },
    ],
  )

  const terminalInputs = [
    {
      expectedKind: 'unknown',
      expectedLabel: 'Unknown',
      job: {
        state: 'completed',
        result: {
          verdict: 'unknown',
          summary: SUMMARY,
          reason: 'time_limit_reached',
        },
      },
    },
    {
      expectedKind: 'cancelled',
      expectedLabel: 'Cancelled',
      job: { state: 'cancelled', summary: SUMMARY },
    },
    {
      expectedKind: 'failed',
      expectedLabel: 'Calculation error',
      job: {
        state: 'failed',
        summary: SUMMARY,
        error_category: 'internal_failure',
      },
    },
    {
      expectedKind: 'stale',
      expectedLabel: 'Outdated result',
      job: { state: 'stale', summary: SUMMARY },
    },
  ] as const
  for (const { expectedKind, expectedLabel, job } of terminalInputs) {
    const presentation = createGlobalFlatFoldabilityPresentation(job, 'en')
    assert.equal(presentation.kind, expectedKind)
    assert.equal(presentation.label, expectedLabel)
    assert.ok(presentation.liveText.length > 0)
  }

  const unknown = createGlobalFlatFoldabilityPresentation(
    terminalInputs[0].job,
    'en',
  )
  assert.equal(
    unknown.resultEntries.at(0)?.label,
    'Reason for indeterminate result',
  )
})

test('English failure presentation never reflects untrusted DTO text', () => {
  const privateValue =
    'C:\\Users\\alice\\秘密\\作品.ori; face_uuid=private; point=(12.3,45.6)'
  const presentation = createGlobalFlatFoldabilityPresentation({
    state: 'failed',
    summary: SUMMARY,
    error_category: 'internal_failure',
    raw_error: privateValue,
  }, 'en')

  assert.equal(presentation.kind, 'failed')
  assert.equal(presentation.label, 'Calculation error')
  assert.equal(
    presentation.detail,
    'The result format could not be verified safely. Its contents are hidden; run the check again with the current edits.',
  )
  assert.doesNotMatch(
    JSON.stringify(presentation),
    /alice|秘密|face_uuid|12\.3|45\.6/iu,
  )
})
