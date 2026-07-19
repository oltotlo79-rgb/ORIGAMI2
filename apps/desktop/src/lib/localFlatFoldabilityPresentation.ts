import type {
  LocalFlatFoldabilityCondition,
  LocalFlatFoldabilityReason,
  LocalFlatFoldabilityReport,
} from './coreClient'
import {
  DEFAULT_LOCALE,
  formatLocalizedText,
  type Locale,
} from './i18n.ts'

export const LOCAL_FLAT_FOLDABILITY_VISIBLE_ITEM_LIMIT = 20
export const LOCAL_FLAT_FOLDABILITY_MODEL =
  'interior_single_vertex_zero_thickness_v1' as const

export type LocalFlatFoldabilityHighlight = 'violated' | 'indeterminate'

export type LocalFlatFoldabilityVertexPresentation = Readonly<{
  vertexId: string
  ordinal: number
  foldDegree: number
  mountainCount: number
  valleyCount: number
  verdict: LocalFlatFoldabilityCondition
  reason: LocalFlatFoldabilityReason
  kawasaki: LocalFlatFoldabilityCondition
  maekawa: LocalFlatFoldabilityCondition
}>

type Counts = Readonly<{
  total: number
  applicable: number
  satisfied: number
  violated: number
  notApplicable: number
  indeterminate: number
}>

type EmptyPresentation = Readonly<{
  maxExactFoldDegree: number | null
  counts: Counts
  verticesById: ReadonlyMap<string, LocalFlatFoldabilityVertexPresentation>
  highlights: ReadonlyMap<string, LocalFlatFoldabilityHighlight>
  visibleItems: readonly LocalFlatFoldabilityVertexPresentation[]
  hiddenItemCount: number
}>

export type LocalFlatFoldabilityPresentation =
  | (EmptyPresentation & Readonly<{
      kind: 'invalid'
      summaryText: string
    }>)
  | (EmptyPresentation & Readonly<{
      kind: 'blocked'
      summaryText: string
    }>)
  | Readonly<{
      kind: 'ready'
      reportStatus: Exclude<LocalFlatFoldabilityReport['status'], 'blocked'>
      maxExactFoldDegree: number
      counts: Counts
      summaryText: string
      verticesById: ReadonlyMap<string, LocalFlatFoldabilityVertexPresentation>
      highlights: ReadonlyMap<string, LocalFlatFoldabilityHighlight>
      visibleItems: readonly LocalFlatFoldabilityVertexPresentation[]
      hiddenItemCount: number
    }>

const EMPTY_COUNTS: Counts = Object.freeze({
  total: 0,
  applicable: 0,
  satisfied: 0,
  violated: 0,
  notApplicable: 0,
  indeterminate: 0,
})

const CONDITION_VALUES = new Set<LocalFlatFoldabilityCondition>([
  'satisfied',
  'violated',
  'not_applicable',
  'indeterminate',
])

const REPORT_STATUS_VALUES = new Set<LocalFlatFoldabilityReport['status']>([
  'blocked',
  'not_applicable',
  'necessary_conditions_satisfied',
  'violated',
  'indeterminate',
])

const REASON_VALUES = new Set<Exclude<LocalFlatFoldabilityReason, null>>([
  'paper_boundary',
  'cut_incident',
  'fold_degree_limit',
  'no_incident_fold_edges',
])

const REPORT_KEYS = [
  'model',
  'max_exact_fold_degree',
  'status',
  'total_vertices',
  'applicable_vertices',
  'satisfied_vertices',
  'violated_vertices',
  'not_applicable_vertices',
  'indeterminate_vertices',
  'vertices',
] as const

const VERTEX_KEYS = [
  'vertex',
  'fold_degree',
  'mountain_count',
  'valley_count',
  'verdict',
  'reason',
  'kawasaki',
  'maekawa',
] as const

export function createLocalFlatFoldabilityPresentation(
  rawReport: unknown,
  currentProjectVertexIds: readonly string[],
  locale: Locale = DEFAULT_LOCALE,
): LocalFlatFoldabilityPresentation {
  try {
    const report = exactDataRecord(rawReport, REPORT_KEYS)
    if (!report) return invalidPresentation(locale)

    const model = report.model
    const maxExactFoldDegree = report.max_exact_fold_degree
    const status = report.status
    const totalVertices = report.total_vertices
    const applicableVertices = report.applicable_vertices
    const satisfiedVertices = report.satisfied_vertices
    const violatedVertices = report.violated_vertices
    const notApplicableVertices = report.not_applicable_vertices
    const indeterminateVertices = report.indeterminate_vertices
    const rawVertices = report.vertices

    if (
      model !== LOCAL_FLAT_FOLDABILITY_MODEL
      || !isNonNegativeSafeInteger(maxExactFoldDegree)
      || !isReportStatus(status)
      || !isNonNegativeSafeInteger(totalVertices)
      || !isNonNegativeSafeInteger(applicableVertices)
      || !isNonNegativeSafeInteger(satisfiedVertices)
      || !isNonNegativeSafeInteger(violatedVertices)
      || !isNonNegativeSafeInteger(notApplicableVertices)
      || !isNonNegativeSafeInteger(indeterminateVertices)
      || !Array.isArray(rawVertices)
    ) return invalidPresentation(locale)

    if (status === 'blocked') {
      if (
        totalVertices !== 0
        || applicableVertices !== 0
        || satisfiedVertices !== 0
        || violatedVertices !== 0
        || notApplicableVertices !== 0
        || indeterminateVertices !== 0
        || rawVertices.length !== 0
      ) return invalidPresentation(locale)
      return blockedPresentation(maxExactFoldDegree, locale)
    }

    const projectVertexIds = snapshotProjectVertexIds(currentProjectVertexIds)
    if (
      !projectVertexIds
      || totalVertices !== projectVertexIds.ids.length
      || rawVertices.length !== projectVertexIds.ids.length
    ) return invalidPresentation(locale)

    const verticesById = new Map<string, LocalFlatFoldabilityVertexPresentation>()
    const highlights = new Map<string, LocalFlatFoldabilityHighlight>()
    const buckets: Array<LocalFlatFoldabilityVertexPresentation[]> = [
      [],
      [],
      [],
      [],
      [],
    ]
    let actualSatisfied = 0
    let actualViolated = 0
    let actualNotApplicable = 0
    let actualIndeterminate = 0

    for (const rawVertex of rawVertices) {
      const vertex = exactDataRecord(rawVertex, VERTEX_KEYS)
      if (!vertex) return invalidPresentation(locale)
      const vertexId = vertex.vertex
      const foldDegree = vertex.fold_degree
      const mountainCount = vertex.mountain_count
      const valleyCount = vertex.valley_count
      const verdict = vertex.verdict
      const reason = vertex.reason
      const kawasaki = vertex.kawasaki
      const maekawa = vertex.maekawa
      if (
        typeof vertexId !== 'string'
        || !projectVertexIds.ordinals.has(vertexId)
        || verticesById.has(vertexId)
        || !isNonNegativeSafeInteger(foldDegree)
        || !isNonNegativeSafeInteger(mountainCount)
        || !isNonNegativeSafeInteger(valleyCount)
        || !isCondition(verdict)
        || !isReason(reason)
        || !isCondition(kawasaki)
        || !isCondition(maekawa)
        || !vertexResultIsConsistent({
          foldDegree,
          mountainCount,
          valleyCount,
          verdict,
          reason,
          kawasaki,
          maekawa,
          maxExactFoldDegree,
        })
      ) return invalidPresentation(locale)

      const presented = Object.freeze({
        vertexId,
        ordinal: projectVertexIds.ordinals.get(vertexId) ?? 0,
        foldDegree,
        mountainCount,
        valleyCount,
        verdict,
        reason,
        kawasaki,
        maekawa,
      })
      verticesById.set(vertexId, presented)
      switch (verdict) {
        case 'satisfied':
          actualSatisfied += 1
          break
        case 'violated':
          actualViolated += 1
          highlights.set(vertexId, 'violated')
          buckets[0].push(presented)
          break
        case 'indeterminate':
          actualIndeterminate += 1
          highlights.set(vertexId, 'indeterminate')
          buckets[1].push(presented)
          break
        case 'not_applicable':
          actualNotApplicable += 1
          buckets[notApplicableBucket(reason)].push(presented)
          break
      }
    }

    if (
      verticesById.size !== projectVertexIds.ids.length
      || satisfiedVertices !== actualSatisfied
      || violatedVertices !== actualViolated
      || notApplicableVertices !== actualNotApplicable
      || indeterminateVertices !== actualIndeterminate
      || totalVertices
        !== actualSatisfied + actualViolated + actualNotApplicable + actualIndeterminate
      || applicableVertices !== actualSatisfied + actualViolated + actualIndeterminate
      || !statusMatchesCounts(
        status,
        applicableVertices,
        actualViolated,
        actualIndeterminate,
      )
    ) return invalidPresentation(locale)

    const visibleItems: LocalFlatFoldabilityVertexPresentation[] = []
    let actionableItemCount = 0
    for (const bucket of buckets) {
      actionableItemCount += bucket.length
      for (const item of bucket) {
        if (visibleItems.length < LOCAL_FLAT_FOLDABILITY_VISIBLE_ITEM_LIMIT) {
          visibleItems.push(item)
        }
      }
    }
    const counts = Object.freeze({
      total: totalVertices,
      applicable: applicableVertices,
      satisfied: satisfiedVertices,
      violated: violatedVertices,
      notApplicable: notApplicableVertices,
      indeterminate: indeterminateVertices,
    })
    return Object.freeze({
      kind: 'ready',
      reportStatus: status,
      maxExactFoldDegree,
      counts,
      summaryText: summaryText(status, counts, locale),
      verticesById,
      highlights,
      visibleItems: Object.freeze(visibleItems),
      hiddenItemCount: actionableItemCount - visibleItems.length,
    })
  } catch {
    return invalidPresentation(locale)
  }
}

export function localFlatFoldabilityConditionLabel(
  condition: LocalFlatFoldabilityCondition,
  locale: Locale = DEFAULT_LOCALE,
) {
  const labels = locale === 'en'
    ? {
      satisfied: 'Satisfied',
      violated: 'Violated',
      not_applicable: 'Not applicable',
      indeterminate: 'Indeterminate',
    }
    : {
      satisfied: '成立',
      violated: '不成立',
      not_applicable: '対象外',
      indeterminate: '判定不能',
    }
  return labels[condition]
}

export function localFlatFoldabilityReasonLabel(
  reason: LocalFlatFoldabilityReason,
  maxExactFoldDegree: number,
  locale: Locale = DEFAULT_LOCALE,
) {
  switch (reason) {
    case 'paper_boundary':
      return localized(
        locale,
        '紙の輪郭頂点は現在の局所条件の対象外です',
        'Paper boundary vertices are outside the current local model.',
      )
    case 'cut_incident':
      return localized(
        locale,
        '切断線に接している頂点は現在の局所条件の対象外です',
        'Vertices incident to a cut line are outside the current local model.',
      )
    case 'fold_degree_limit':
      return formatLocalizedText(locale, {
        ja: '折り線次数が厳密計算上限（{limit}）を超えたため判定不能です',
        en: 'Indeterminate because the fold degree exceeds the exact limit ({limit}).',
      }, { limit: maxExactFoldDegree })
    case 'no_incident_fold_edges':
      return localized(
        locale,
        '判定対象の山折り・谷折り線がないため対象外です',
        'Not applicable because there are no incident mountain or valley folds.',
      )
    case null:
      return ''
  }
}

function invalidPresentation(
  locale: Locale,
): LocalFlatFoldabilityPresentation {
  return Object.freeze({
    kind: 'invalid',
    maxExactFoldDegree: null,
    counts: EMPTY_COUNTS,
    summaryText: localized(
      locale,
      '局所平坦折り条件の結果を確認できませんでした。成立とは扱いません。',
      'The local flat-foldability result could not be verified and is not treated as satisfied.',
    ),
    verticesById: new Map(),
    highlights: new Map(),
    visibleItems: Object.freeze([]),
    hiddenItemCount: 0,
  })
}

function blockedPresentation(
  maxExactFoldDegree: number,
  locale: Locale,
): LocalFlatFoldabilityPresentation {
  return Object.freeze({
    kind: 'blocked',
    maxExactFoldDegree,
    counts: EMPTY_COUNTS,
    summaryText: localized(
      locale,
      '前段の幾何構造に問題があるため、局所平坦折り条件は判定していません。',
      'Local flat-foldability conditions were not checked because the preceding geometry is invalid.',
    ),
    verticesById: new Map(),
    highlights: new Map(),
    visibleItems: Object.freeze([]),
    hiddenItemCount: 0,
  })
}

function snapshotProjectVertexIds(vertexIds: readonly string[]) {
  if (!Array.isArray(vertexIds)) return null
  const ids: string[] = []
  const ordinals = new Map<string, number>()
  for (const vertexId of vertexIds) {
    if (typeof vertexId !== 'string' || vertexId.length === 0 || ordinals.has(vertexId)) {
      return null
    }
    ids.push(vertexId)
    ordinals.set(vertexId, ids.length)
  }
  return { ids, ordinals }
}

function exactDataRecord<const Keys extends readonly string[]>(
  value: unknown,
  expectedKeys: Keys,
): { [Key in Keys[number]]: unknown } | null {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return null
  const prototype = Object.getPrototypeOf(value)
  if (prototype !== Object.prototype && prototype !== null) return null
  const descriptors = Object.getOwnPropertyDescriptors(value)
  const keys = Object.keys(descriptors)
  if (keys.length !== expectedKeys.length) return null
  const expected = new Set<string>(expectedKeys)
  const record: Record<string, unknown> = {}
  for (const key of keys) {
    const descriptor = descriptors[key]
    if (!expected.has(key) || !descriptor || !('value' in descriptor)) return null
    record[key] = descriptor.value
  }
  return record as { [Key in Keys[number]]: unknown }
}

function isNonNegativeSafeInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0
}

function isCondition(value: unknown): value is LocalFlatFoldabilityCondition {
  return typeof value === 'string'
    && CONDITION_VALUES.has(value as LocalFlatFoldabilityCondition)
}

function isReportStatus(value: unknown): value is LocalFlatFoldabilityReport['status'] {
  return typeof value === 'string'
    && REPORT_STATUS_VALUES.has(value as LocalFlatFoldabilityReport['status'])
}

function isReason(value: unknown): value is LocalFlatFoldabilityReason {
  return value === null
    || (typeof value === 'string'
      && REASON_VALUES.has(value as Exclude<LocalFlatFoldabilityReason, null>))
}

function vertexResultIsConsistent(input: Readonly<{
  foldDegree: number
  mountainCount: number
  valleyCount: number
  verdict: LocalFlatFoldabilityCondition
  reason: LocalFlatFoldabilityReason
  kawasaki: LocalFlatFoldabilityCondition
  maekawa: LocalFlatFoldabilityCondition
  maxExactFoldDegree: number
}>) {
  const assignmentCount = input.mountainCount + input.valleyCount
  if (!Number.isSafeInteger(assignmentCount) || input.foldDegree !== assignmentCount) return false

  if (input.verdict === 'not_applicable') {
    const reasonIsConsistent = input.reason === 'paper_boundary'
      || input.reason === 'cut_incident'
      || (
        input.reason === 'no_incident_fold_edges'
        && input.foldDegree === 0
      )
    return reasonIsConsistent
      && input.kawasaki === 'not_applicable'
      && input.maekawa === 'not_applicable'
  }

  if (input.foldDegree === 0) return false
  const expectedMaekawa = Math.abs(input.mountainCount - input.valleyCount) === 2
    ? 'satisfied'
    : 'violated'
  if (input.maekawa !== expectedMaekawa) return false

  const kawasakiIsConsistent = input.foldDegree % 2 !== 0
    ? input.kawasaki === 'violated'
    : input.foldDegree > input.maxExactFoldDegree
      ? input.kawasaki === 'indeterminate'
      : input.kawasaki === 'satisfied' || input.kawasaki === 'violated'
  if (!kawasakiIsConsistent) return false

  const expectedVerdict = input.kawasaki === 'violated'
    || input.maekawa === 'violated'
    ? 'violated'
    : input.kawasaki === 'indeterminate'
      ? 'indeterminate'
      : 'satisfied'
  const expectedReason = expectedVerdict === 'indeterminate'
    ? 'fold_degree_limit'
    : null
  return input.verdict === expectedVerdict && input.reason === expectedReason
}

function notApplicableBucket(reason: LocalFlatFoldabilityReason) {
  if (reason === 'paper_boundary') return 2
  if (reason === 'cut_incident') return 3
  return 4
}

function statusMatchesCounts(
  status: Exclude<LocalFlatFoldabilityReport['status'], 'blocked'>,
  applicable: number,
  violated: number,
  indeterminate: number,
) {
  switch (status) {
    case 'necessary_conditions_satisfied':
      return applicable > 0 && violated === 0 && indeterminate === 0
    case 'not_applicable':
      return applicable === 0
    case 'violated':
      return violated > 0
    case 'indeterminate':
      return violated === 0 && indeterminate > 0
  }
}

function summaryText(
  status: Exclude<LocalFlatFoldabilityReport['status'], 'blocked'>,
  counts: Counts,
  locale: Locale,
) {
  const detail = formatLocalizedText(locale, {
    ja: '成立{satisfied}、不成立{violated}、対象外{notApplicable}、判定不能{indeterminate}',
    en: 'satisfied {satisfied}, violated {violated}, not applicable {notApplicable}, indeterminate {indeterminate}',
  }, {
    satisfied: counts.satisfied,
    violated: counts.violated,
    notApplicable: counts.notApplicable,
    indeterminate: counts.indeterminate,
  })
  switch (status) {
    case 'necessary_conditions_satisfied':
      return formatLocalizedText(locale, {
        ja: '対応範囲内の局所必要条件が成立しました（{detail}）。',
        en: 'The supported local necessary conditions are satisfied ({detail}).',
      }, { detail })
    case 'not_applicable':
      return formatLocalizedText(locale, {
        ja: '現在の局所条件を適用できる頂点がありません（{detail}）。',
        en: 'No vertices are eligible for the current local conditions ({detail}).',
      }, { detail })
    case 'violated':
      return formatLocalizedText(locale, {
        ja: '局所必要条件に不成立の頂点があります（{detail}）。',
        en: 'At least one vertex violates the local necessary conditions ({detail}).',
      }, { detail })
    case 'indeterminate':
      return formatLocalizedText(locale, {
        ja: '局所必要条件を判定できない頂点があります（{detail}）。',
        en: 'At least one vertex has indeterminate local necessary conditions ({detail}).',
      }, { detail })
  }
}

function localized(locale: Locale, ja: string, en: string): string {
  return locale === 'en' ? en : ja
}
