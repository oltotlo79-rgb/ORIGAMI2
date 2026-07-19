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
import {
  DEFAULT_LOCALE,
  formatLocalizedText,
  type Locale,
} from './i18n.ts'

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

export function createGlobalFlatFoldabilityPresentation(
  rawJob: unknown,
  locale: Locale = DEFAULT_LOCALE,
): GlobalFlatFoldabilityPresentation {
  if (rawJob === null) return idlePresentation(locale)
  const job = parseGlobalFlatFoldabilityJobDto(rawJob)
  if (!job) return invalidPresentation(locale)

  switch (job.state) {
    case 'queued':
      return activePresentation(job, 'queued', locale)
    case 'running':
      return activePresentation(job, 'running', locale)
    case 'cancelled':
      return terminalPresentation({
        kind: 'cancelled',
        icon: '■',
        label: localized(locale, '中止', 'Cancelled'),
        detail: localized(
          locale,
          '全体平坦折り判定を中止しました。判定途中の候補は採用していません。',
          'The global flat-foldability check was cancelled. Intermediate candidates were not accepted.',
        ),
        liveText: localized(
          locale,
          '全体平坦折り判定を中止しました。',
          'The global flat-foldability check was cancelled.',
        ),
        summary: job.summary,
      }, locale)
    case 'failed':
      return terminalPresentation({
        kind: 'failed',
        icon: '!',
        label: localized(locale, '計算エラー', 'Calculation error'),
        detail: globalFlatFoldabilityErrorMessage(job.error_category, locale),
        liveText: localized(
          locale,
          '全体平坦折り判定は計算エラーで終了しました。',
          'The global flat-foldability check ended with a calculation error.',
        ),
        summary: job.summary,
      }, locale)
    case 'stale':
      return terminalPresentation({
        kind: 'stale',
        icon: '↻',
        label: localized(locale, '古い結果', 'Outdated result'),
        detail: localized(
          locale,
          '判定開始後に編集内容が変わったため、この結果は現在の作品へ適用できません。現在の内容で再判定してください。',
          'The project changed after the check started, so this result does not apply to the current work. Run the check again.',
        ),
        liveText: localized(
          locale,
          '全体平坦折り判定の結果は古いため、現在の作品へ適用できません。',
          'The global flat-foldability result is outdated and does not apply to the current work.',
        ),
        summary: job.summary,
      }, locale)
    case 'completed':
      return completedPresentation(job.result, locale)
  }
}

export function globalFlatFoldabilityPhaseLabel(
  phase: GlobalFlatFoldabilityPhase,
  locale: Locale = DEFAULT_LOCALE,
) {
  switch (phase) {
    case 'capturing':
      return localized(locale, '編集内容を取得しています', 'Capturing edits')
    case 'validating_local_conditions':
      return localized(
        locale,
        '局所平坦折り条件を確認しています',
        'Checking local flat-foldability conditions',
      )
    case 'building_flat_embedding':
      return localized(
        locale,
        '平面配置を構築しています',
        'Building the flat embedding',
      )
    case 'building_overlap_arrangement':
      return localized(
        locale,
        '重なり領域を構築しています',
        'Building overlap regions',
      )
    case 'building_constraints':
      return localized(
        locale,
        '層順序の制約を構築しています',
        'Building layer-order constraints',
      )
    case 'propagating':
      return localized(
        locale,
        '確定した層順序を伝播しています',
        'Propagating determined layer order',
      )
    case 'searching':
      return localized(locale, '層順序を探索しています', 'Searching layer order')
    case 'verifying_certificate':
      return localized(
        locale,
        '判定根拠を再検証しています',
        'Verifying the result certificate',
      )
    case 'completed':
      return localized(locale, '判定結果を確定しています', 'Finalizing the result')
  }
}

export function globalFlatFoldabilityUnknownReasonMessage(
  reason: GlobalFlatFoldabilityUnknownReason,
  locale: Locale = DEFAULT_LOCALE,
) {
  switch (reason) {
    case 'unsupported_topology':
      return localized(
        locale,
        '切断、穴、未接続材料など、初版の対象外となる面構造が含まれています。',
        'The face structure includes cuts, holes, disconnected material, or another topology outside the initial release scope.',
      )
    case 'non_convex_face':
      return localized(
        locale,
        '凸多角形でない面があるため、初版の対象として判定できませんでした。',
        'The check is indeterminate because at least one face is not a convex polygon supported by the initial release.',
      )
    case 'time_limit_reached':
      return localized(
        locale,
        '選択した時間制限内に証明を完了できませんでした。時間を延ばして再判定できます。',
        'The proof could not be completed within the selected time limit. Choose a longer limit and run the check again.',
      )
    case 'work_limit_reached':
      return localized(
        locale,
        '処理件数が初版の作業上限に達したため、証明を完了できませんでした。',
        'The proof could not be completed because the initial-release work limit was reached.',
      )
    case 'exact_number_limit_reached':
      return localized(
        locale,
        '正確な数値計算が初版の安全上限に達したため、証明を完了できませんでした。',
        'The proof could not be completed because exact arithmetic reached the initial-release safety limit.',
      )
    case 'overlap_arrangement_limit_reached':
      return localized(
        locale,
        '重なり領域の構築が初版の安全上限に達したため、証明を完了できませんでした。',
        'The proof could not be completed because overlap-region construction reached the initial-release safety limit.',
      )
    case 'constraint_limit_reached':
      return localized(
        locale,
        '層順序の制約数が初版の安全上限に達したため、証明を完了できませんでした。',
        'The proof could not be completed because the number of layer-order constraints reached the initial-release safety limit.',
      )
    case 'proof_not_completed':
      return localized(
        locale,
        '可または不可を確定できる証明を完成できませんでした。',
        'A proof establishing Possible or Impossible could not be completed.',
      )
    case 'local_conditions_indeterminate':
      return localized(
        locale,
        '局所平坦折り条件に未確定の頂点があるため、全体判定を確定できませんでした。',
        'The global result is indeterminate because at least one vertex has an indeterminate local flat-foldability condition.',
      )
  }
}

export function globalFlatFoldabilityProofLabel(
  category: GlobalFlatFoldabilityProofCategory,
  locale: Locale = DEFAULT_LOCALE,
) {
  switch (category) {
    case 'local_conditions_violated':
      return localized(
        locale,
        '局所必要条件の明示的な違反',
        'Explicit violation of local necessary conditions',
      )
    case 'inconsistent_flat_embedding':
      return localized(
        locale,
        '平面配置の経路間矛盾',
        'Path inconsistency in the flat embedding',
      )
    case 'layer_constraints_contradictory':
      return localized(
        locale,
        '層順序制約の矛盾',
        'Contradictory layer-order constraints',
      )
    case 'exhaustive_search_no_solution':
      return localized(
        locale,
        '全候補の探索完了（解なし）',
        'Exhaustive search completed (no solution)',
      )
  }
}

export function globalFlatFoldabilityErrorMessage(
  category: GlobalFlatFoldabilityErrorCategory,
  locale: Locale = DEFAULT_LOCALE,
) {
  switch (category) {
    case 'invalid_request':
      return localized(
        locale,
        '判定を開始するための条件を確認できませんでした。現在の編集内容で再試行してください。',
        'The conditions required to start the check could not be verified. Retry with the current edits.',
      )
    case 'snapshot_unavailable':
      return localized(
        locale,
        '判定用の編集内容を安全に取得できませんでした。現在の編集内容で再試行してください。',
        'The edits required for the check could not be captured safely. Retry with the current edits.',
      )
    case 'worker_unavailable':
      return localized(
        locale,
        '判定処理を開始できませんでした。少し待ってから再試行してください。',
        'The check could not be started. Wait briefly and retry.',
      )
    case 'result_unavailable':
      return localized(
        locale,
        '完了した判定結果を安全に取得できませんでした。再判定してください。',
        'The completed result could not be retrieved safely. Run the check again.',
      )
    case 'internal_failure':
      return localized(
        locale,
        '判定処理を安全に完了できませんでした。作品は変更されていません。',
        'The check could not be completed safely. The work was not changed.',
      )
  }
}

function idlePresentation(locale: Locale): GlobalFlatFoldabilityPresentation {
  return Object.freeze({
    kind: 'idle',
    icon: '◇',
    label: localized(locale, '未判定', 'Not checked'),
    detail: localized(
      locale,
      '時間制限を選び、現在の編集内容について判定を開始できます。',
      'Select a time limit to check the current edits.',
    ),
    liveText: '',
    active: false,
    cancelRequested: false,
    phaseText: null,
    workText: null,
    summaryEntries: staticSummaryEntries(locale),
    resultEntries: Object.freeze([]),
  })
}

function invalidPresentation(locale: Locale): GlobalFlatFoldabilityPresentation {
  return Object.freeze({
    kind: 'failed',
    icon: '!',
    label: localized(locale, '計算エラー', 'Calculation error'),
    detail: localized(
      locale,
      '判定結果の形式を安全に確認できませんでした。内容は表示せず、現在の編集内容で再判定できます。',
      'The result format could not be verified safely. Its contents are hidden; run the check again with the current edits.',
    ),
    liveText: localized(
      locale,
      '全体平坦折り判定の結果を安全に確認できませんでした。',
      'The global flat-foldability result could not be verified safely.',
    ),
    active: false,
    cancelRequested: false,
    phaseText: null,
    workText: null,
    summaryEntries: staticSummaryEntries(locale),
    resultEntries: Object.freeze([]),
  })
}

function activePresentation(
  job: Extract<GlobalFlatFoldabilityJobDto, { state: 'queued' | 'running' }>,
  kind: 'queued' | 'running',
  locale: Locale,
): GlobalFlatFoldabilityPresentation {
  const phaseText = globalFlatFoldabilityPhaseLabel(job.progress.phase, locale)
  const workText = formatProgressWork(
    job.progress.completed_work,
    job.progress.total_work,
    locale,
  )
  const cancelText = job.cancel_requested
    ? localized(
      locale,
      '中止しています。処理が安全に終了するまでお待ちください。',
      'Cancelling. Wait for the process to end safely.',
    )
    : kind === 'queued'
      ? localized(
        locale,
        '判定開始を待っています。',
        'Waiting for the check to start.',
      )
      : localized(
        locale,
        '判定中も展開図の編集を続けられます。',
        'You can continue editing the crease pattern while the check runs.',
      )
  const label = job.cancel_requested
    ? localized(locale, '中止しています', 'Cancelling')
    : kind === 'queued'
      ? localized(locale, '開始待ち', 'Queued')
      : localized(locale, '判定中', 'Checking')
  return Object.freeze({
    kind,
    icon: job.cancel_requested ? '■' : kind === 'queued' ? '○' : '▶',
    label,
    detail: cancelText,
    // Work counts remain visible, but a 250 ms poll must not repeatedly
    // interrupt screen readers. Announce only state/cancel and phase changes.
    liveText: formatLocalizedText(locale, {
      ja: '{label}。{phase}。',
      en: '{label}. {phase}.',
    }, { label, phase: phaseText }),
    active: true,
    cancelRequested: job.cancel_requested,
    phaseText,
    workText,
    summaryEntries: summaryEntries(job.progress, locale),
    resultEntries: Object.freeze([]),
  })
}

function completedPresentation(
  result: Extract<GlobalFlatFoldabilityJobDto, { state: 'completed' }>['result'],
  locale: Locale,
): GlobalFlatFoldabilityPresentation {
  switch (result.verdict) {
    case 'possible':
      return terminalPresentation({
        kind: 'possible',
        icon: '✓',
        label: localized(locale, '可', 'Possible'),
        detail: localized(
          locale,
          '理想的な厚さ0のモデルで、条件を満たす層順序を構成し、判定根拠を再検証できました。',
          'A layer order satisfying the conditions was constructed and its certificate was verified for the ideal zero-thickness model.',
        ),
        liveText: localized(
          locale,
          '全体平坦折り判定の結果は、可です。',
          'The global flat-foldability result is Possible.',
        ),
        summary: result.summary,
        resultEntries: [
          {
            label: localized(locale, '層順序モデル', 'Layer-order model'),
            value: result.layer_order.model_id,
          },
          {
            label: localized(locale, '層数', 'Layer count'),
            value: formatLocalizedText(locale, {
              ja: '{count}層',
              en: '{count} layers',
            }, { count: formatCount(result.layer_order.layer_count, locale) }),
          },
          {
            label: localized(locale, '最大重なり', 'Maximum overlap'),
            value: `${formatCount(result.layer_order.max_ply, locale)} ply`,
          },
          {
            label: localized(locale, '基準面', 'Reference face'),
            value: formatLocalizedText(locale, {
              ja: '面 {number}',
              en: 'Face {number}',
            }, {
              number: formatCount(
                result.layer_order.reference_face_number,
                locale,
              ),
            }),
          },
          {
            label: localized(locale, '層順3D表示', '3D layer-order view'),
            value: result.layer_order.layer_view_available
              ? localized(locale, '利用できます', 'Available')
              : localized(locale, '利用できません', 'Unavailable'),
          },
        ],
      }, locale)
    case 'impossible': {
      const numberedFaces = result.proof.face_numbers
        .map((faceNumber) => formatLocalizedText(locale, {
          ja: '面 {number}',
          en: 'Face {number}',
        }, { number: formatCount(faceNumber, locale) }))
        .join(locale === 'ja' ? '、' : ', ')
      const faceText = result.proof.category === 'exhaustive_search_no_solution'
        ? result.proof.face_numbers.length < result.summary.counts.face_count
          ? formatLocalizedText(locale, {
            ja: '全体：{faces}（ほか{remaining}面）',
            en: 'All: {faces} ({remaining} more faces)',
          }, {
            faces: numberedFaces,
            remaining: formatCount(
              result.summary.counts.face_count - result.proof.face_numbers.length,
              locale,
            ),
          })
          : formatLocalizedText(locale, {
            ja: '全体：{faces}',
            en: 'All: {faces}',
          }, { faces: numberedFaces })
        : numberedFaces
      return terminalPresentation({
        kind: 'impossible',
        icon: '✕',
        label: localized(locale, '不可', 'Impossible'),
        detail: localized(
          locale,
          '初版の対象クラス内で、平坦折り可能な層順序が存在しないことを有限の根拠で確認しました。',
          'Finite evidence established that no flat-foldable layer order exists within the initial-release target class.',
        ),
        liveText: localized(
          locale,
          '全体平坦折り判定の結果は、不可です。',
          'The global flat-foldability result is Impossible.',
        ),
        summary: result.summary,
        resultEntries: [
          {
            label: localized(locale, '証明種別', 'Proof type'),
            value: globalFlatFoldabilityProofLabel(
              result.proof.category,
              locale,
            ),
          },
          {
            label: localized(
              locale,
              '対象面（FaceKey順・最大20件）',
              'Target faces (FaceKey order, up to 20)',
            ),
            value: faceText,
          },
        ],
      }, locale)
    }
    case 'unknown': {
      const reason = globalFlatFoldabilityUnknownReasonMessage(
        result.reason,
        locale,
      )
      return terminalPresentation({
        kind: 'unknown',
        icon: '?',
        label: localized(locale, '不明', 'Unknown'),
        detail: reason,
        liveText: formatLocalizedText(locale, {
          ja: '全体平坦折り判定の結果は、不明です。{reason}',
          en: 'The global flat-foldability result is Unknown. {reason}',
        }, { reason }),
        summary: result.summary,
        resultEntries: [{
          label: localized(
            locale,
            '確定できない理由',
            'Reason for indeterminate result',
          ),
          value: reason,
        }],
      }, locale)
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
}>, locale: Locale): GlobalFlatFoldabilityPresentation {
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
    summaryEntries: summaryEntries(input.summary, locale),
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
  locale: Locale,
): readonly GlobalFlatFoldabilityPresentationEntry[] {
  return Object.freeze([
    ...staticSummaryEntries(locale),
    Object.freeze({
      label: localized(locale, '経過時間', 'Elapsed time'),
      value: formatElapsedMilliseconds(input.elapsed_ms, locale),
    }),
    Object.freeze({
      label: localized(locale, '面', 'Faces'),
      value: formatItemCount(input.counts.face_count, locale),
    }),
    Object.freeze({
      label: localized(locale, '重なりcell', 'Overlap cells'),
      value: formatItemCount(input.counts.overlap_cell_count, locale),
    }),
    Object.freeze({
      label: localized(locale, '制約', 'Constraints'),
      value: formatItemCount(input.counts.constraint_count, locale),
    }),
    Object.freeze({
      label: localized(locale, '探索node', 'Search nodes'),
      value: formatItemCount(input.counts.search_node_count, locale),
    }),
  ])
}

function staticSummaryEntries(
  locale: Locale,
): readonly GlobalFlatFoldabilityPresentationEntry[] {
  return Object.freeze([
    Object.freeze({
      label: localized(locale, '判定モデル', 'Check model'),
      value: GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
    }),
    Object.freeze({
      label: localized(locale, '対象クラス', 'Target class'),
      value: locale === 'ja'
        ? GLOBAL_FLAT_FOLDABILITY_TARGET_CLASS
        : 'Convex polygonal faces (no cuts, holes, or disconnected material)',
    }),
  ])
}

function formatProgressWork(
  completed: number,
  total: number | null,
  locale: Locale,
) {
  return total === null
    ? formatLocalizedText(locale, {
      ja: '{completed}件完了（総数は計算中）',
      en: '{completed} completed (total still being calculated)',
    }, { completed: formatCount(completed, locale) })
    : formatLocalizedText(locale, {
      ja: '{completed} / {total}件完了',
      en: '{completed} / {total} completed',
    }, {
      completed: formatCount(completed, locale),
      total: formatCount(total, locale),
    })
}

function formatItemCount(value: number, locale: Locale) {
  return formatLocalizedText(locale, {
    ja: '{count}件',
    en: '{count}',
  }, { count: formatCount(value, locale) })
}

function formatCount(value: number, locale: Locale) {
  return value.toLocaleString(locale === 'ja' ? 'ja-JP' : 'en-US')
}

function formatElapsedMilliseconds(milliseconds: number, locale: Locale) {
  if (milliseconds < 1_000) {
    return formatLocalizedText(locale, {
      ja: '{milliseconds}ミリ秒',
      en: '{milliseconds} ms',
    }, { milliseconds: formatCount(milliseconds, locale) })
  }
  if (milliseconds < 60_000) {
    const seconds = Math.round(milliseconds / 100) / 10
    return formatLocalizedText(locale, {
      ja: '{seconds}秒',
      en: '{seconds} s',
    }, {
      seconds: seconds.toLocaleString(locale === 'ja' ? 'ja-JP' : 'en-US', {
        maximumFractionDigits: 1,
      }),
    })
  }
  const minutes = Math.floor(milliseconds / 60_000)
  const seconds = Math.floor((milliseconds % 60_000) / 1_000)
  return seconds === 0
    ? formatLocalizedText(locale, {
      ja: '{minutes}分',
      en: '{minutes} min',
    }, { minutes: formatCount(minutes, locale) })
    : formatLocalizedText(locale, {
      ja: '{minutes}分{seconds}秒',
      en: '{minutes} min {seconds} s',
    }, {
      minutes: formatCount(minutes, locale),
      seconds: formatCount(seconds, locale),
    })
}

function localized(locale: Locale, ja: string, en: string): string {
  return locale === 'en' ? en : ja
}
