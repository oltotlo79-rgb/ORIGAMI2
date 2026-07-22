import { isCanonicalNonNilUuid as isCanonicalUuid } from './canonicalUuid.ts'
import {
  DEFAULT_LOCALE,
  formatLocalizedText,
  type Locale,
} from './i18n.ts'

export const GEOMETRIC_CONSTRAINT_SCHEMA_VERSION = 1 as const
export const MAX_GEOMETRIC_CONSTRAINT_RECORDS = 10_000
export const MAX_GEOMETRIC_CONSTRAINT_REFERENCES = 40_000
export const MAX_GEOMETRIC_CONSTRAINT_DIRECT_CONFLICTS = 10_000
export const MAX_DIRECT_CONFLICT_WITNESS_IDS = 256

export type GeometricConstraintKindV1 =
  | Readonly<{
      kind: 'fixed_length'
      edge: string
      length_mm: number
    }>
  | Readonly<{
      kind: 'fixed_angle'
      vertex: string
      first_edge: string
      second_edge: string
      angle_degrees: number
    }>
  | Readonly<{
      kind: 'horizontal'
      edge: string
    }>
  | Readonly<{
      kind: 'vertical'
      edge: string
    }>
  | Readonly<{
      kind: 'equal_length'
      first_edge: string
      second_edge: string
    }>
  | Readonly<{
      kind: 'parallel'
      first_edge: string
      second_edge: string
    }>
  | Readonly<{
      kind: 'point_on_line'
      vertex: string
      line_edge: string
    }>
  | Readonly<{
      kind: 'mirror_symmetry'
      first_vertex: string
      second_vertex: string
      axis_edge: string
    }>
  | Readonly<{
      kind: 'rotational_symmetry'
      center_vertex: string
      source_vertex: string
      target_vertex: string
      angle_degrees: number
    }>
  | Readonly<{
      kind: 'angle_bisector'
      vertex: string
      first_edge: string
      second_edge: string
      bisector_edge: string
    }>
  | Readonly<{
      kind: 'length_ratio'
      numerator_edge: string
      denominator_edge: string
      ratio: number
    }>

export type GeometricConstraintRecordV1 = Readonly<{
  id: string
  constraint: GeometricConstraintKindV1
}>

export type GeometricConstraintDocumentV1 = Readonly<{
  schema_version: typeof GEOMETRIC_CONSTRAINT_SCHEMA_VERSION
  constraints: readonly GeometricConstraintRecordV1[]
}>

export type DirectConstraintConflictKindV1 =
  | Readonly<{
      kind: 'different_fixed_lengths'
      edge: string
    }>
  | Readonly<{
      kind: 'different_fixed_angles'
      vertex: string
      first_edge: string
      second_edge: string
    }>
  | Readonly<{
      kind: 'different_length_ratios'
      numerator_edge: string
      denominator_edge: string
    }>
  | Readonly<{
      kind: 'horizontal_and_vertical'
      edge: string
    }>
  | Readonly<{
      kind: 'equal_length_with_different_fixed_lengths'
      first_edge: string
      second_edge: string
    }>
  | Readonly<{
      kind: 'equal_length_with_non_unit_ratio_and_fixed_length'
      first_edge: string
      second_edge: string
    }>
  | Readonly<{
      kind: 'non_reciprocal_length_ratios_with_fixed_length'
      first_edge: string
      second_edge: string
    }>
  | Readonly<{
      kind: 'length_ratio_with_incompatible_fixed_lengths'
      numerator_edge: string
      denominator_edge: string
    }>
  | Readonly<{
      kind: 'non_unit_length_ratio_cycle_with_fixed_length'
      first_edge: string
      second_edge: string
      third_edge: string
      fixed_edge: string
    }>
  | Readonly<{
      kind: 'inconsistent_length_ratio_graph_with_fixed_length'
      fixed_edge: string
      ratio_constraint_count: number
    }>
  | Readonly<{
      kind: 'different_fixed_lengths_in_equal_length_component'
      first_edge: string
      second_edge: string
      equal_constraint_count: number
    }>
  | Readonly<{
      kind: 'parallel_with_fixed_non_parallel_angle'
      first_edge: string
      second_edge: string
    }>
  | Readonly<{
      kind: 'parallel_with_perpendicular_orientations'
      horizontal_edge: string
      vertical_edge: string
    }>

export type DirectConstraintConflictV1 = Readonly<{
  conflict: DirectConstraintConflictKindV1
  constraint_ids: readonly string[]
}>

export type GeometricConstraintUnknownReasonV1 =
  | 'work_limit_exceeded'
  | 'solver_required_constraint_kinds'
  | 'invalid_document_or_geometry'

export type GeometricConstraintPreflightResultV1 =
  | Readonly<{
      status: 'direct_conflict'
      conflicts: readonly DirectConstraintConflictV1[]
    }>
  | Readonly<{
      status: 'no_direct_conflict'
    }>
  | Readonly<{
      status: 'unknown'
      reason: GeometricConstraintUnknownReasonV1
      unchecked_constraint_ids: readonly string[]
    }>

export type GeometricConstraintPreflightBinding = Readonly<{
  project_instance_id: string
  project_id: string
  revision: number
}>

export type GeometricConstraintPreflightResponseV1 =
  GeometricConstraintPreflightBinding
  & Readonly<{
    result: GeometricConstraintPreflightResultV1
  }>

export type GeometricConstraintPresentation = Readonly<{
  constraintId: string
  kind: GeometricConstraintKindV1['kind']
  displayName: string
  targetSummary: string
}>

/**
 * Detaches an untrusted native/persisted DTO into the exact V1 document
 * contract. Invalid input, accessors, proxy failures, and resource-limit
 * violations all fail closed as `null`.
 */
export function normalizeGeometricConstraintDocument(
  value: unknown,
): GeometricConstraintDocumentV1 | null {
  try {
    const document = exactDataRecord(value, ['schema_version', 'constraints'])
    if (
      !document
      || document.schema_version !== GEOMETRIC_CONSTRAINT_SCHEMA_VERSION
    ) return null
    const source = snapshotExactArray(
      document.constraints,
      MAX_GEOMETRIC_CONSTRAINT_RECORDS,
    )
    if (!source) return null

    const ids = new Set<string>()
    const constraints: GeometricConstraintRecordV1[] = []
    let referenceCount = 0
    for (const rawRecord of source) {
      const record = parseConstraintRecord(rawRecord)
      if (!record || ids.has(record.id)) return null
      ids.add(record.id)
      referenceCount += constraintReferenceCount(record.constraint)
      if (
        !Number.isSafeInteger(referenceCount)
        || referenceCount > MAX_GEOMETRIC_CONSTRAINT_REFERENCES
      ) return null
      constraints.push(record)
    }

    return Object.freeze({
      schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION,
      constraints: Object.freeze(constraints),
    })
  } catch {
    return null
  }
}

export function normalizeGeometricConstraintKind(
  value: unknown,
): GeometricConstraintKindV1 | null {
  try {
    return parseConstraint(value)
  } catch {
    return null
  }
}

/**
 * Accepts a preflight response only for the exact project snapshot requested
 * by the caller. No raw native diagnostic or geometry is retained.
 */
export function normalizeGeometricConstraintPreflightResponse(
  value: unknown,
  expectedBinding: unknown,
): GeometricConstraintPreflightResponseV1 | null {
  try {
    const expected = parsePreflightBinding(expectedBinding)
    const response = exactDataRecord(value, [
      'project_instance_id',
      'project_id',
      'revision',
      'result',
    ])
    if (
      !expected
      || !response
      || !isCanonicalUuid(response.project_instance_id)
      || !isCanonicalUuid(response.project_id)
      || !isRevision(response.revision)
      || response.project_instance_id !== expected.project_instance_id
      || response.project_id !== expected.project_id
      || response.revision !== expected.revision
    ) return null
    const result = parsePreflightResult(response.result)
    if (!result) return null

    return Object.freeze({
      project_instance_id: response.project_instance_id,
      project_id: response.project_id,
      revision: response.revision,
      result,
    })
  } catch {
    return null
  }
}

/**
 * Produces fixed localized UI copy from one valid record. The presentation
 * contains only its kind and opaque target identities—never coordinates,
 * native errors, or geometry snapshots.
 */
export function createGeometricConstraintPresentation(
  value: unknown,
  locale: Locale = DEFAULT_LOCALE,
): GeometricConstraintPresentation | null {
  try {
    const record = parseConstraintRecord(value)
    if (!record) return null
    const constraint = record.constraint
    switch (constraint.kind) {
      case 'fixed_length':
        return presentation(
          record.id,
          constraint.kind,
          localized(locale, '長さを固定', 'Fixed length'),
          formatLocalizedText(locale, {
            ja: '辺 {edge}',
            en: 'Edge {edge}',
          }, { edge: constraint.edge }),
        )
      case 'fixed_angle':
        return presentation(
          record.id,
          constraint.kind,
          localized(locale, '角度を固定', 'Fixed angle'),
          formatLocalizedText(locale, {
            ja: '頂点 {vertex}／辺 {firstEdge}・{secondEdge}',
            en: 'Vertex {vertex} / Edges {firstEdge} · {secondEdge}',
          }, {
            vertex: constraint.vertex,
            firstEdge: constraint.first_edge,
            secondEdge: constraint.second_edge,
          }),
        )
      case 'horizontal':
        return presentation(
          record.id,
          constraint.kind,
          localized(locale, '水平', 'Horizontal'),
          formatLocalizedText(locale, {
            ja: '辺 {edge}',
            en: 'Edge {edge}',
          }, { edge: constraint.edge }),
        )
      case 'vertical':
        return presentation(
          record.id,
          constraint.kind,
          localized(locale, '垂直', 'Vertical'),
          formatLocalizedText(locale, {
            ja: '辺 {edge}',
            en: 'Edge {edge}',
          }, { edge: constraint.edge }),
        )
      case 'equal_length':
        return presentation(
          record.id,
          constraint.kind,
          localized(locale, '等しい長さ', 'Equal length'),
          formatLocalizedText(locale, {
            ja: '辺 {firstEdge}・{secondEdge}',
            en: 'Edges {firstEdge} · {secondEdge}',
          }, {
            firstEdge: constraint.first_edge,
            secondEdge: constraint.second_edge,
          }),
        )
      case 'parallel':
        return presentation(
          record.id,
          constraint.kind,
          localized(locale, '平行', 'Parallel'),
          formatLocalizedText(locale, {
            ja: '辺 {firstEdge}・{secondEdge}',
            en: 'Edges {firstEdge} · {secondEdge}',
          }, {
            firstEdge: constraint.first_edge,
            secondEdge: constraint.second_edge,
          }),
        )
      case 'point_on_line':
        return presentation(
          record.id,
          constraint.kind,
          localized(locale, '点を直線上に配置', 'Point on line'),
          formatLocalizedText(locale, {
            ja: '頂点 {vertex}／直線 {edge}',
            en: 'Vertex {vertex} / Line {edge}',
          }, {
            vertex: constraint.vertex,
            edge: constraint.line_edge,
          }),
        )
      case 'mirror_symmetry':
        return presentation(
          record.id,
          constraint.kind,
          localized(locale, '線対称', 'Mirror symmetry'),
          formatLocalizedText(locale, {
            ja: '頂点 {firstVertex}・{secondVertex}／対称軸 {axisEdge}',
            en: 'Vertices {firstVertex} · {secondVertex} / Symmetry axis {axisEdge}',
          }, {
            firstVertex: constraint.first_vertex,
            secondVertex: constraint.second_vertex,
            axisEdge: constraint.axis_edge,
          }),
        )
      case 'rotational_symmetry':
        return presentation(
          record.id,
          constraint.kind,
          localized(locale, '回転対称', 'Rotational symmetry'),
          formatLocalizedText(locale, {
            ja: '中心 {centerVertex}／対応する頂点 {sourceVertex}・{targetVertex}',
            en: 'Center {centerVertex} / Corresponding vertices {sourceVertex} · {targetVertex}',
          }, {
            centerVertex: constraint.center_vertex,
            sourceVertex: constraint.source_vertex,
            targetVertex: constraint.target_vertex,
          }),
        )
      case 'angle_bisector':
        return presentation(
          record.id,
          constraint.kind,
          localized(locale, '角の二等分', 'Angle bisector'),
          formatLocalizedText(locale, {
            ja: '頂点 {vertex}／角の辺 {firstEdge}・{secondEdge}／二等分線 {bisectorEdge}',
            en: 'Vertex {vertex} / Angle edges {firstEdge} · {secondEdge} / Bisector {bisectorEdge}',
          }, {
            vertex: constraint.vertex,
            firstEdge: constraint.first_edge,
            secondEdge: constraint.second_edge,
            bisectorEdge: constraint.bisector_edge,
          }),
        )
      case 'length_ratio':
        return presentation(
          record.id,
          constraint.kind,
          localized(locale, '長さの比', 'Length ratio'),
          formatLocalizedText(locale, {
            ja: '分子の辺 {numeratorEdge}／分母の辺 {denominatorEdge}',
            en: 'Numerator edge {numeratorEdge} / Denominator edge {denominatorEdge}',
          }, {
            numeratorEdge: constraint.numerator_edge,
            denominatorEdge: constraint.denominator_edge,
          }),
        )
    }
  } catch {
    return null
  }
}

function localized(locale: Locale, ja: string, en: string): string {
  return locale === 'en' ? en : ja
}

function presentation(
  constraintId: string,
  kind: GeometricConstraintKindV1['kind'],
  displayName: string,
  targetSummary: string,
): GeometricConstraintPresentation {
  return Object.freeze({
    constraintId,
    kind,
    displayName,
    targetSummary,
  })
}

function parseConstraintRecord(value: unknown): GeometricConstraintRecordV1 | null {
  const record = exactDataRecord(value, ['id', 'constraint'])
  if (!record || !isCanonicalUuid(record.id)) return null
  const constraint = parseConstraint(record.constraint)
  if (!constraint) return null
  return Object.freeze({ id: record.id, constraint })
}

function parseConstraint(value: unknown): GeometricConstraintKindV1 | null {
  const record = snapshotDataRecord(value)
  if (!record || typeof record.kind !== 'string') return null
  switch (record.kind) {
    case 'fixed_length':
      if (
        !hasExactKeys(record, ['kind', 'edge', 'length_mm'])
        || !isCanonicalUuid(record.edge)
        || !isPositiveFinite(record.length_mm)
      ) return null
      return Object.freeze({
        kind: record.kind,
        edge: record.edge,
        length_mm: record.length_mm,
      })
    case 'fixed_angle':
      if (
        !hasExactKeys(record, [
          'kind',
          'vertex',
          'first_edge',
          'second_edge',
          'angle_degrees',
        ])
        || !isCanonicalUuid(record.vertex)
        || !isCanonicalUuid(record.first_edge)
        || !isCanonicalUuid(record.second_edge)
        || record.first_edge === record.second_edge
        || !isClosedAngle(record.angle_degrees)
      ) return null
      return Object.freeze({
        kind: record.kind,
        vertex: record.vertex,
        first_edge: record.first_edge,
        second_edge: record.second_edge,
        angle_degrees: normalizeZero(record.angle_degrees),
      })
    case 'horizontal':
    case 'vertical':
      if (
        !hasExactKeys(record, ['kind', 'edge'])
        || !isCanonicalUuid(record.edge)
      ) return null
      return Object.freeze({
        kind: record.kind,
        edge: record.edge,
      })
    case 'equal_length':
    case 'parallel':
      if (
        !hasExactKeys(record, ['kind', 'first_edge', 'second_edge'])
        || !isCanonicalUuid(record.first_edge)
        || !isCanonicalUuid(record.second_edge)
        || record.first_edge === record.second_edge
      ) return null
      return Object.freeze({
        kind: record.kind,
        first_edge: record.first_edge,
        second_edge: record.second_edge,
      })
    case 'point_on_line':
      if (
        !hasExactKeys(record, ['kind', 'vertex', 'line_edge'])
        || !isCanonicalUuid(record.vertex)
        || !isCanonicalUuid(record.line_edge)
      ) return null
      return Object.freeze({
        kind: record.kind,
        vertex: record.vertex,
        line_edge: record.line_edge,
      })
    case 'mirror_symmetry':
      if (
        !hasExactKeys(record, [
          'kind',
          'first_vertex',
          'second_vertex',
          'axis_edge',
        ])
        || !isCanonicalUuid(record.first_vertex)
        || !isCanonicalUuid(record.second_vertex)
        || record.first_vertex === record.second_vertex
        || !isCanonicalUuid(record.axis_edge)
      ) return null
      return Object.freeze({
        kind: record.kind,
        first_vertex: record.first_vertex,
        second_vertex: record.second_vertex,
        axis_edge: record.axis_edge,
      })
    case 'rotational_symmetry':
      if (
        !hasExactKeys(record, [
          'kind',
          'center_vertex',
          'source_vertex',
          'target_vertex',
          'angle_degrees',
        ])
        || !isCanonicalUuid(record.center_vertex)
        || !isCanonicalUuid(record.source_vertex)
        || !isCanonicalUuid(record.target_vertex)
        || !allDistinct([
          record.center_vertex,
          record.source_vertex,
          record.target_vertex,
        ])
        || !isOpenRotationAngle(record.angle_degrees)
      ) return null
      return Object.freeze({
        kind: record.kind,
        center_vertex: record.center_vertex,
        source_vertex: record.source_vertex,
        target_vertex: record.target_vertex,
        angle_degrees: record.angle_degrees,
      })
    case 'angle_bisector':
      if (
        !hasExactKeys(record, [
          'kind',
          'vertex',
          'first_edge',
          'second_edge',
          'bisector_edge',
        ])
        || !isCanonicalUuid(record.vertex)
        || !isCanonicalUuid(record.first_edge)
        || !isCanonicalUuid(record.second_edge)
        || !isCanonicalUuid(record.bisector_edge)
        || !allDistinct([
          record.first_edge,
          record.second_edge,
          record.bisector_edge,
        ])
      ) return null
      return Object.freeze({
        kind: record.kind,
        vertex: record.vertex,
        first_edge: record.first_edge,
        second_edge: record.second_edge,
        bisector_edge: record.bisector_edge,
      })
    case 'length_ratio':
      if (
        !hasExactKeys(record, [
          'kind',
          'numerator_edge',
          'denominator_edge',
          'ratio',
        ])
        || !isCanonicalUuid(record.numerator_edge)
        || !isCanonicalUuid(record.denominator_edge)
        || record.numerator_edge === record.denominator_edge
        || !isPositiveFinite(record.ratio)
      ) return null
      return Object.freeze({
        kind: record.kind,
        numerator_edge: record.numerator_edge,
        denominator_edge: record.denominator_edge,
        ratio: record.ratio,
      })
    default:
      return null
  }
}

function constraintReferenceCount(constraint: GeometricConstraintKindV1): number {
  switch (constraint.kind) {
    case 'fixed_length':
    case 'horizontal':
    case 'vertical':
      return 1
    case 'equal_length':
    case 'parallel':
    case 'point_on_line':
    case 'length_ratio':
      return 2
    case 'fixed_angle':
    case 'mirror_symmetry':
    case 'rotational_symmetry':
      return 3
    case 'angle_bisector':
      return 4
  }
}

function parsePreflightBinding(
  value: unknown,
): GeometricConstraintPreflightBinding | null {
  const record = exactDataRecord(value, [
    'project_instance_id',
    'project_id',
    'revision',
  ])
  if (
    !record
    || !isCanonicalUuid(record.project_instance_id)
    || !isCanonicalUuid(record.project_id)
    || !isRevision(record.revision)
  ) return null
  return Object.freeze({
    project_instance_id: record.project_instance_id,
    project_id: record.project_id,
    revision: record.revision,
  })
}

function parsePreflightResult(
  value: unknown,
): GeometricConstraintPreflightResultV1 | null {
  const record = snapshotDataRecord(value)
  if (!record || typeof record.status !== 'string') return null
  switch (record.status) {
    case 'no_direct_conflict':
      return hasExactKeys(record, ['status'])
        ? Object.freeze({ status: record.status })
        : null
    case 'unknown': {
      if (
        !hasExactKeys(record, [
          'status',
          'reason',
          'unchecked_constraint_ids',
        ])
        || !isUnknownReason(record.reason)
      ) return null
      const ids = parseSortedUniqueUuidArray(
        record.unchecked_constraint_ids,
        MAX_GEOMETRIC_CONSTRAINT_RECORDS,
      )
      if (!ids) return null
      return Object.freeze({
        status: record.status,
        reason: record.reason,
        unchecked_constraint_ids: ids,
      })
    }
    case 'direct_conflict': {
      if (!hasExactKeys(record, ['status', 'conflicts'])) return null
      const source = snapshotExactArray(
        record.conflicts,
        MAX_GEOMETRIC_CONSTRAINT_DIRECT_CONFLICTS,
      )
      if (!source || source.length === 0) return null
      const conflicts: DirectConstraintConflictV1[] = []
      const conflictKeys = new Set<string>()
      for (const rawConflict of source) {
        const conflict = parseDirectConflict(rawConflict)
        if (!conflict) return null
        const key = directConflictKey(conflict)
        if (conflictKeys.has(key)) return null
        conflictKeys.add(key)
        conflicts.push(conflict)
      }
      return Object.freeze({
        status: record.status,
        conflicts: Object.freeze(conflicts),
      })
    }
    default:
      return null
  }
}

function parseDirectConflict(value: unknown): DirectConstraintConflictV1 | null {
  const record = exactDataRecord(value, ['conflict', 'constraint_ids'])
  if (!record) return null
  const parsed = parseDirectConflictKind(record.conflict)
  if (!parsed) return null
  const constraintIds = parseSortedUniqueUuidArray(
    record.constraint_ids,
    MAX_DIRECT_CONFLICT_WITNESS_IDS,
  )
  if (!constraintIds || constraintIds.length !== parsed.witnessSize) return null
  return Object.freeze({
    conflict: parsed.conflict,
    constraint_ids: constraintIds,
  })
}

function parseDirectConflictKind(
  value: unknown,
): Readonly<{
  conflict: DirectConstraintConflictKindV1
  witnessSize: number
}> | null {
  const record = snapshotDataRecord(value)
  if (!record || typeof record.kind !== 'string') return null
  switch (record.kind) {
    case 'different_fixed_lengths':
      if (
        !hasExactKeys(record, ['kind', 'edge'])
        || !isCanonicalUuid(record.edge)
      ) return null
      return {
        conflict: Object.freeze({
          kind: record.kind,
          edge: record.edge,
        }),
        witnessSize: 2,
      }
    case 'horizontal_and_vertical':
      if (
        !hasExactKeys(record, ['kind', 'edge'])
        || !isCanonicalUuid(record.edge)
      ) return null
      return {
        conflict: Object.freeze({
          kind: record.kind,
          edge: record.edge,
        }),
        witnessSize: 3,
      }
    case 'different_fixed_angles':
      if (
        !hasExactKeys(record, [
          'kind',
          'vertex',
          'first_edge',
          'second_edge',
        ])
        || !isCanonicalUuid(record.vertex)
        || !isCanonicalUuid(record.first_edge)
        || !isCanonicalUuid(record.second_edge)
        || record.first_edge === record.second_edge
      ) return null
      return {
        conflict: Object.freeze({
          kind: record.kind,
          vertex: record.vertex,
          first_edge: record.first_edge,
          second_edge: record.second_edge,
        }),
        witnessSize: 2,
      }
    case 'different_length_ratios':
      if (
        !hasExactKeys(record, [
          'kind',
          'numerator_edge',
          'denominator_edge',
        ])
        || !isCanonicalUuid(record.numerator_edge)
        || !isCanonicalUuid(record.denominator_edge)
        || record.numerator_edge === record.denominator_edge
      ) return null
      return {
        conflict: Object.freeze({
          kind: record.kind,
          numerator_edge: record.numerator_edge,
          denominator_edge: record.denominator_edge,
        }),
        witnessSize: 2,
      }
    case 'equal_length_with_different_fixed_lengths':
    case 'equal_length_with_non_unit_ratio_and_fixed_length':
    case 'non_reciprocal_length_ratios_with_fixed_length':
      if (
        !hasExactKeys(record, ['kind', 'first_edge', 'second_edge'])
        || !isCanonicalUuid(record.first_edge)
        || !isCanonicalUuid(record.second_edge)
        || record.first_edge === record.second_edge
      ) return null
      return {
        conflict: Object.freeze({
          kind: record.kind,
          first_edge: record.first_edge,
          second_edge: record.second_edge,
        }),
        witnessSize: 3,
      }
    case 'length_ratio_with_incompatible_fixed_lengths':
      if (
        !hasExactKeys(record, [
          'kind',
          'numerator_edge',
          'denominator_edge',
        ])
        || !isCanonicalUuid(record.numerator_edge)
        || !isCanonicalUuid(record.denominator_edge)
        || record.numerator_edge === record.denominator_edge
      ) return null
      return {
        conflict: Object.freeze({
          kind: record.kind,
          numerator_edge: record.numerator_edge,
          denominator_edge: record.denominator_edge,
        }),
        witnessSize: 3,
      }
    case 'non_unit_length_ratio_cycle_with_fixed_length':
      if (
        !hasExactKeys(record, [
          'kind',
          'first_edge',
          'second_edge',
          'third_edge',
          'fixed_edge',
        ])
        || !isCanonicalUuid(record.first_edge)
        || !isCanonicalUuid(record.second_edge)
        || !isCanonicalUuid(record.third_edge)
        || !isCanonicalUuid(record.fixed_edge)
        || new Set([
          record.first_edge,
          record.second_edge,
          record.third_edge,
        ]).size !== 3
        || ![
          record.first_edge,
          record.second_edge,
          record.third_edge,
        ].includes(record.fixed_edge)
      ) return null
      return {
        conflict: Object.freeze({
          kind: record.kind,
          first_edge: record.first_edge,
          second_edge: record.second_edge,
          third_edge: record.third_edge,
          fixed_edge: record.fixed_edge,
        }),
        witnessSize: 4,
      }
    case 'inconsistent_length_ratio_graph_with_fixed_length':
      if (
        !hasExactKeys(record, [
          'kind',
          'fixed_edge',
          'ratio_constraint_count',
        ])
        || !isCanonicalUuid(record.fixed_edge)
        || typeof record.ratio_constraint_count !== 'number'
        || !Number.isSafeInteger(record.ratio_constraint_count)
        || record.ratio_constraint_count < 3
        || record.ratio_constraint_count >= MAX_DIRECT_CONFLICT_WITNESS_IDS
      ) return null
      return {
        conflict: Object.freeze({
          kind: record.kind,
          fixed_edge: record.fixed_edge,
          ratio_constraint_count: record.ratio_constraint_count,
        }),
        witnessSize: record.ratio_constraint_count + 1,
      }
    case 'different_fixed_lengths_in_equal_length_component':
      if (
        !hasExactKeys(record, [
          'kind',
          'first_edge',
          'second_edge',
          'equal_constraint_count',
        ])
        || !isCanonicalUuid(record.first_edge)
        || !isCanonicalUuid(record.second_edge)
        || record.first_edge === record.second_edge
        || typeof record.equal_constraint_count !== 'number'
        || !Number.isSafeInteger(record.equal_constraint_count)
        || record.equal_constraint_count < 2
        || record.equal_constraint_count > MAX_DIRECT_CONFLICT_WITNESS_IDS - 2
      ) return null
      return {
        conflict: Object.freeze({
          kind: record.kind,
          first_edge: record.first_edge,
          second_edge: record.second_edge,
          equal_constraint_count: record.equal_constraint_count,
        }),
        witnessSize: record.equal_constraint_count + 2,
      }
    case 'parallel_with_fixed_non_parallel_angle':
      if (
        !hasExactKeys(record, ['kind', 'first_edge', 'second_edge'])
        || !isCanonicalUuid(record.first_edge)
        || !isCanonicalUuid(record.second_edge)
        || record.first_edge === record.second_edge
      ) return null
      return {
        conflict: Object.freeze({
          kind: record.kind,
          first_edge: record.first_edge,
          second_edge: record.second_edge,
        }),
        witnessSize: 2,
      }
    case 'parallel_with_perpendicular_orientations':
      if (
        !hasExactKeys(record, [
          'kind',
          'horizontal_edge',
          'vertical_edge',
        ])
        || !isCanonicalUuid(record.horizontal_edge)
        || !isCanonicalUuid(record.vertical_edge)
        || record.horizontal_edge === record.vertical_edge
      ) return null
      return {
        conflict: Object.freeze({
          kind: record.kind,
          horizontal_edge: record.horizontal_edge,
          vertical_edge: record.vertical_edge,
        }),
        witnessSize: 3,
      }
    default:
      return null
  }
}

function parseSortedUniqueUuidArray(
  value: unknown,
  maximum: number,
): readonly string[] | null {
  const source = snapshotExactArray(value, maximum)
  if (!source) return null
  const ids: string[] = []
  let previous: string | null = null
  for (const id of source) {
    if (
      !isCanonicalUuid(id)
      || (previous !== null && compareCodeUnits(previous, id) >= 0)
    ) return null
    ids.push(id)
    previous = id
  }
  return Object.freeze(ids)
}

function directConflictKey(conflict: DirectConstraintConflictV1): string {
  const kind = conflict.conflict
  let target: readonly string[]
  switch (kind.kind) {
    case 'different_fixed_lengths':
    case 'horizontal_and_vertical':
      target = [kind.kind, kind.edge]
      break
    case 'different_fixed_angles':
      target = [
        kind.kind,
        kind.vertex,
        kind.first_edge,
        kind.second_edge,
      ]
      break
    case 'different_length_ratios':
    case 'length_ratio_with_incompatible_fixed_lengths':
      target = [kind.kind, kind.numerator_edge, kind.denominator_edge]
      break
    case 'non_unit_length_ratio_cycle_with_fixed_length':
      target = [
        kind.kind,
        kind.first_edge,
        kind.second_edge,
        kind.third_edge,
        kind.fixed_edge,
      ]
      break
    case 'inconsistent_length_ratio_graph_with_fixed_length':
      target = [
        kind.kind,
        kind.fixed_edge,
        String(kind.ratio_constraint_count),
      ]
      break
    case 'different_fixed_lengths_in_equal_length_component':
      target = [
        kind.kind,
        kind.first_edge,
        kind.second_edge,
        String(kind.equal_constraint_count),
      ]
      break
    case 'equal_length_with_different_fixed_lengths':
    case 'equal_length_with_non_unit_ratio_and_fixed_length':
    case 'non_reciprocal_length_ratios_with_fixed_length':
    case 'parallel_with_fixed_non_parallel_angle':
      target = [kind.kind, kind.first_edge, kind.second_edge]
      break
    case 'parallel_with_perpendicular_orientations':
      target = [kind.kind, kind.horizontal_edge, kind.vertical_edge]
      break
  }
  return `${target.join('\u0000')}\u0001${conflict.constraint_ids.join('\u0000')}`
}

function snapshotDataRecord(
  value: unknown,
): Record<string, unknown> | null {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    return null
  }
  const prototype = Object.getPrototypeOf(value)
  if (prototype !== Object.prototype && prototype !== null) return null
  const descriptors = Object.getOwnPropertyDescriptors(value)
  const keys = Reflect.ownKeys(descriptors)
  const snapshot = Object.create(null) as Record<string, unknown>
  for (const key of keys) {
    if (typeof key !== 'string') return null
    const descriptor = descriptors[key]
    if (
      !descriptor
      || !('value' in descriptor)
      || !descriptor.enumerable
    ) return null
    snapshot[key] = descriptor.value
  }
  return snapshot
}

function exactDataRecord<const Keys extends readonly string[]>(
  value: unknown,
  keys: Keys,
): Readonly<Record<Keys[number], unknown>> | null {
  const record = snapshotDataRecord(value)
  return record && hasExactKeys(record, keys)
    ? record as Readonly<Record<Keys[number], unknown>>
    : null
}

function hasExactKeys(
  record: Readonly<Record<string, unknown>>,
  expected: readonly string[],
): boolean {
  const actual = Object.keys(record)
  return actual.length === expected.length
    && expected.every((key) => Object.hasOwn(record, key))
}

function snapshotExactArray(
  value: unknown,
  maximum: number,
): unknown[] | null {
  if (!Array.isArray(value)) return null
  const descriptors = Object.getOwnPropertyDescriptors(value) as unknown as
    Record<PropertyKey, PropertyDescriptor>
  const keys = Reflect.ownKeys(descriptors)
  if (keys.some((key) => typeof key !== 'string')) return null
  const lengthDescriptor = descriptors.length
  if (
    !lengthDescriptor
    || !('value' in lengthDescriptor)
    || lengthDescriptor.enumerable
    || !Number.isSafeInteger(lengthDescriptor.value)
    || lengthDescriptor.value < 0
    || lengthDescriptor.value > maximum
    || keys.length !== lengthDescriptor.value + 1
  ) return null

  const result: unknown[] = []
  for (let index = 0; index < lengthDescriptor.value; index += 1) {
    const descriptor = descriptors[String(index)]
    if (
      !descriptor
      || !('value' in descriptor)
      || !descriptor.enumerable
    ) return null
    result.push(descriptor.value)
  }
  return result
}

function isRevision(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isSafeInteger(value)
    && value >= 0
}

function isPositiveFinite(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isFinite(value)
    && value > 0
}

function isClosedAngle(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isFinite(value)
    && value >= 0
    && value <= 180
}

function isOpenRotationAngle(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isFinite(value)
    && value > 0
    && value < 360
}

function isUnknownReason(
  value: unknown,
): value is GeometricConstraintUnknownReasonV1 {
  return value === 'work_limit_exceeded'
    || value === 'solver_required_constraint_kinds'
    || value === 'invalid_document_or_geometry'
}

function allDistinct(values: readonly string[]): boolean {
  return new Set(values).size === values.length
}

function compareCodeUnits(left: string, right: string): number {
  return left < right ? -1 : left > right ? 1 : 0
}

function normalizeZero(value: number): number {
  return Object.is(value, -0) ? 0 : value
}
