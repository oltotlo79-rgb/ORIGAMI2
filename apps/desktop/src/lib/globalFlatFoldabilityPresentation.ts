import {
  GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
  GLOBAL_FLAT_FOLDABILITY_TARGET_CLASS,
  parseGlobalFlatFoldabilityJobDto,
  type GlobalFlatFoldabilityErrorCategory,
  type GlobalFlatFoldabilityJobDto,
  type GlobalFlatFoldabilityPhase,
  type GlobalFlatFoldabilityProofCategory,
  type GlobalFlatFoldabilitySummary,
  type GlobalFlatFoldabilityUnknownReason,
} from './globalFlatFoldability.ts'

export type GlobalFlatFoldabilityPresentationKind =
  | 'idle'
  | 'queued'
  | 'running'
  | 'possible'
  | 'impossible'
  | 'unknown'
  | 'cancelled'
  | 'failed'
  | 'stale'

export type GlobalFlatFoldabilityPresentationEntry = Readonly<{
  label: string
  value: string
}>

export type GlobalFlatFoldabilityPresentation = Readonly<{
  kind: GlobalFlatFoldabilityPresentationKind
  icon: string
  label: string
  detail: string
  liveText: string
  active: boolean
  cancelRequested: boolean
  phaseText: string | null
  workText: string | null
  summaryEntries: readonly GlobalFlatFoldabilityPresentationEntry[]
  resultEntries: readonly GlobalFlatFoldabilityPresentationEntry[]
}>

const STATIC_SUMMARY_ENTRIES = Object.freeze([
  Object.freeze({
    label: '判定モデル',
    value: GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
  }),
  Object.freeze({
    label: '対象クラス',
    value: GLOBAL_FLAT_FOLDABILITY_TARGET_CLASS,
  }),
] as const)

export function createGlobalFlatFoldabilityPresentation(
  rawJob: unknown,
): GlobalFlatFoldabilityPresentation {
  if (rawJob === null) return idlePresentation()
  const job = parseGlobalFlatFoldabilityJobDto(rawJob)
  if (!job) return invalidPresentation()

  switch (job.state) {
    case 'queued':
      return activePresentation(job, 'queued')
    case 'running':
      return activePresentation(job, 'running')
    case 'cancelled':
      return terminalPresentation({
        kind: 'cancelled',
        icon: '■',
        label: '中止',
        detail: '全体平坦折り判定を中止しました。判定途中の候補は採用していません。',
        liveText: '全体平坦折り判定を中止しました。',
        summary: job.summary,
      })
    case 'failed':
      return terminalPresentation({
        kind: 'failed',
        icon: '!',
        label: '計算エラー',
        detail: globalFlatFoldabilityErrorMessage(job.error_category),
        liveText: '全体平坦折り判定は計算エラーで終了しました。',
        summary: job.summary,
      })
    case 'stale':
      return terminalPresentation({
        kind: 'stale',
        icon: '↻',
        label: '古い結果',
        detail:
          '判定開始後に編集内容が変わったため、この結果は現在の作品へ適用できません。現在の内容で再判定してください。',
        liveText: '全体平坦折り判定の結果は古いため、現在の作品へ適用できません。',
        summary: job.summary,
      })
    case 'completed':
      return completedPresentation(job.result)
  }
}

export function globalFlatFoldabilityPhaseLabel(
  phase: GlobalFlatFoldabilityPhase,
) {
  switch (phase) {
    case 'capturing':
      return '編集内容を取得しています'
    case 'validating_local_conditions':
      return '局所平坦折り条件を確認しています'
    case 'building_flat_embedding':
      return '平面配置を構築しています'
    case 'building_overlap_arrangement':
      return '重なり領域を構築しています'
    case 'building_constraints':
      return '層順序の制約を構築しています'
    case 'propagating':
      return '確定した層順序を伝播しています'
    case 'searching':
      return '層順序を探索しています'
    case 'verifying_certificate':
      return '判定根拠を再検証しています'
    case 'completed':
      return '判定結果を確定しています'
  }
}

export function globalFlatFoldabilityUnknownReasonMessage(
  reason: GlobalFlatFoldabilityUnknownReason,
) {
  switch (reason) {
    case 'unsupported_topology':
      return '切断、穴、未接続材料など、初版の対象外となる面構造が含まれています。'
    case 'non_convex_face':
      return '凸多角形でない面があるため、初版の対象として判定できませんでした。'
    case 'time_limit_reached':
      return '選択した時間制限内に証明を完了できませんでした。時間を延ばして再判定できます。'
    case 'work_limit_reached':
      return '処理件数が初版の作業上限に達したため、証明を完了できませんでした。'
    case 'exact_number_limit_reached':
      return '正確な数値計算が初版の安全上限に達したため、証明を完了できませんでした。'
    case 'overlap_arrangement_limit_reached':
      return '重なり領域の構築が初版の安全上限に達したため、証明を完了できませんでした。'
    case 'constraint_limit_reached':
      return '層順序の制約数が初版の安全上限に達したため、証明を完了できませんでした。'
    case 'proof_not_completed':
      return '可または不可を確定できる証明を完成できませんでした。'
    case 'local_conditions_indeterminate':
      return '局所平坦折り条件に未確定の頂点があるため、全体判定を確定できませんでした。'
  }
}

export function globalFlatFoldabilityProofLabel(
  category: GlobalFlatFoldabilityProofCategory,
) {
  switch (category) {
    case 'local_conditions_violated':
      return '局所必要条件の明示的な違反'
    case 'inconsistent_flat_embedding':
      return '平面配置の経路間矛盾'
    case 'layer_constraints_contradictory':
      return '層順序制約の矛盾'
    case 'exhaustive_search_no_solution':
      return '全候補の探索完了（解なし）'
  }
}

export function globalFlatFoldabilityErrorMessage(
  category: GlobalFlatFoldabilityErrorCategory,
) {
  switch (category) {
    case 'invalid_request':
      return '判定を開始するための条件を確認できませんでした。現在の編集内容で再試行してください。'
    case 'snapshot_unavailable':
      return '判定用の編集内容を安全に取得できませんでした。現在の編集内容で再試行してください。'
    case 'worker_unavailable':
      return '判定処理を開始できませんでした。少し待ってから再試行してください。'
    case 'result_unavailable':
      return '完了した判定結果を安全に取得できませんでした。再判定してください。'
    case 'internal_failure':
      return '判定処理を安全に完了できませんでした。作品は変更されていません。'
  }
}

function idlePresentation(): GlobalFlatFoldabilityPresentation {
  return Object.freeze({
    kind: 'idle',
    icon: '◇',
    label: '未判定',
    detail: '時間制限を選び、現在の編集内容について判定を開始できます。',
    liveText: '',
    active: false,
    cancelRequested: false,
    phaseText: null,
    workText: null,
    summaryEntries: STATIC_SUMMARY_ENTRIES,
    resultEntries: Object.freeze([]),
  })
}

function invalidPresentation(): GlobalFlatFoldabilityPresentation {
  return Object.freeze({
    kind: 'failed',
    icon: '!',
    label: '計算エラー',
    detail:
      '判定結果の形式を安全に確認できませんでした。内容は表示せず、現在の編集内容で再判定できます。',
    liveText: '全体平坦折り判定の結果を安全に確認できませんでした。',
    active: false,
    cancelRequested: false,
    phaseText: null,
    workText: null,
    summaryEntries: STATIC_SUMMARY_ENTRIES,
    resultEntries: Object.freeze([]),
  })
}

function activePresentation(
  job: Extract<GlobalFlatFoldabilityJobDto, { state: 'queued' | 'running' }>,
  kind: 'queued' | 'running',
): GlobalFlatFoldabilityPresentation {
  const phaseText = globalFlatFoldabilityPhaseLabel(job.progress.phase)
  const workText = formatProgressWork(
    job.progress.completed_work,
    job.progress.total_work,
  )
  const cancelText = job.cancel_requested
    ? '中止しています。処理が安全に終了するまでお待ちください。'
    : kind === 'queued'
      ? '判定開始を待っています。'
      : '判定中も展開図の編集を続けられます。'
  const label = job.cancel_requested
    ? '中止しています'
    : kind === 'queued'
      ? '開始待ち'
      : '判定中'
  return Object.freeze({
    kind,
    icon: job.cancel_requested ? '■' : kind === 'queued' ? '○' : '▶',
    label,
    detail: cancelText,
    // Work counts remain visible, but a 250 ms poll must not repeatedly
    // interrupt screen readers. Announce only state/cancel and phase changes.
    liveText: `${label}。${phaseText}。`,
    active: true,
    cancelRequested: job.cancel_requested,
    phaseText,
    workText,
    summaryEntries: summaryEntries(job.progress),
    resultEntries: Object.freeze([]),
  })
}

function completedPresentation(
  result: Extract<GlobalFlatFoldabilityJobDto, { state: 'completed' }>['result'],
): GlobalFlatFoldabilityPresentation {
  switch (result.verdict) {
    case 'possible':
      return terminalPresentation({
        kind: 'possible',
        icon: '✓',
        label: '可',
        detail:
          '理想的な厚さ0のモデルで、条件を満たす層順序を構成し、判定根拠を再検証できました。',
        liveText: '全体平坦折り判定の結果は、可です。',
        summary: result.summary,
        resultEntries: [
          {
            label: '層順序モデル',
            value: result.layer_order.model_id,
          },
          {
            label: '層数',
            value: `${formatCount(result.layer_order.layer_count)}層`,
          },
          {
            label: '最大重なり',
            value: `${formatCount(result.layer_order.max_ply)} ply`,
          },
          {
            label: '基準面',
            value: `面 ${formatCount(result.layer_order.reference_face_number)}`,
          },
          {
            label: '層順3D表示',
            value: result.layer_order.layer_view_available ? '利用できます' : '利用できません',
          },
        ],
      })
    case 'impossible': {
      const numberedFaces = result.proof.face_numbers
        .map((faceNumber) => `面 ${formatCount(faceNumber)}`)
        .join('、')
      const faceText = result.proof.category === 'exhaustive_search_no_solution'
        ? result.proof.face_numbers.length < result.summary.counts.face_count
          ? `全体：${numberedFaces}（ほか${
            formatCount(
              result.summary.counts.face_count - result.proof.face_numbers.length,
            )
          }面）`
          : `全体：${numberedFaces}`
        : numberedFaces
      return terminalPresentation({
        kind: 'impossible',
        icon: '✕',
        label: '不可',
        detail:
          '初版の対象クラス内で、平坦折り可能な層順序が存在しないことを有限の根拠で確認しました。',
        liveText: '全体平坦折り判定の結果は、不可です。',
        summary: result.summary,
        resultEntries: [
          {
            label: '証明種別',
            value: globalFlatFoldabilityProofLabel(result.proof.category),
          },
          { label: '対象面（FaceKey順・最大20件）', value: faceText },
        ],
      })
    }
    case 'unknown': {
      const reason = globalFlatFoldabilityUnknownReasonMessage(result.reason)
      return terminalPresentation({
        kind: 'unknown',
        icon: '?',
        label: '不明',
        detail: reason,
        liveText: `全体平坦折り判定の結果は、不明です。${reason}`,
        summary: result.summary,
        resultEntries: [{ label: '確定できない理由', value: reason }],
      })
    }
  }
}

function terminalPresentation(input: Readonly<{
  kind: Extract<
    GlobalFlatFoldabilityPresentationKind,
    'possible' | 'impossible' | 'unknown' | 'cancelled' | 'failed' | 'stale'
  >
  icon: string
  label: string
  detail: string
  liveText: string
  summary: GlobalFlatFoldabilitySummary
  resultEntries?: readonly GlobalFlatFoldabilityPresentationEntry[]
}>): GlobalFlatFoldabilityPresentation {
  return Object.freeze({
    kind: input.kind,
    icon: input.icon,
    label: input.label,
    detail: input.detail,
    liveText: input.liveText,
    active: false,
    cancelRequested: false,
    phaseText: null,
    workText: null,
    summaryEntries: summaryEntries(input.summary),
    resultEntries: Object.freeze(
      (input.resultEntries ?? []).map((entry) => Object.freeze(entry)),
    ),
  })
}

function summaryEntries(
  input: GlobalFlatFoldabilitySummary | Extract<
    GlobalFlatFoldabilityJobDto,
    { state: 'queued' | 'running' }
  >['progress'],
): readonly GlobalFlatFoldabilityPresentationEntry[] {
  return Object.freeze([
    ...STATIC_SUMMARY_ENTRIES,
    Object.freeze({
      label: '経過時間',
      value: formatElapsedMilliseconds(input.elapsed_ms),
    }),
    Object.freeze({
      label: '面',
      value: `${formatCount(input.counts.face_count)}件`,
    }),
    Object.freeze({
      label: '重なりcell',
      value: `${formatCount(input.counts.overlap_cell_count)}件`,
    }),
    Object.freeze({
      label: '制約',
      value: `${formatCount(input.counts.constraint_count)}件`,
    }),
    Object.freeze({
      label: '探索node',
      value: `${formatCount(input.counts.search_node_count)}件`,
    }),
  ])
}

function formatProgressWork(completed: number, total: number | null) {
  return total === null
    ? `${formatCount(completed)}件完了（総数は計算中）`
    : `${formatCount(completed)} / ${formatCount(total)}件完了`
}

function formatCount(value: number) {
  return value.toLocaleString('ja-JP')
}

function formatElapsedMilliseconds(milliseconds: number) {
  if (milliseconds < 1_000) return `${formatCount(milliseconds)}ミリ秒`
  if (milliseconds < 60_000) {
    const seconds = Math.round(milliseconds / 100) / 10
    return `${seconds.toLocaleString('ja-JP', {
      maximumFractionDigits: 1,
    })}秒`
  }
  const minutes = Math.floor(milliseconds / 60_000)
  const seconds = Math.floor((milliseconds % 60_000) / 1_000)
  return seconds === 0
    ? `${formatCount(minutes)}分`
    : `${formatCount(minutes)}分${formatCount(seconds)}秒`
}
