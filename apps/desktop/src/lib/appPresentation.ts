import type { CreaseLine } from '../components/CreaseCanvas'
import {
  creasePatternExportFormatLabel,
  type CreasePatternExportFormat,
} from './creaseExport'
import {
  instructionExportFormatLabel,
  type InstructionExportFormat,
} from './instructionExport'
import {
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
  type LocalizedText,
} from './i18n'
import {
  localFlatFoldabilityConditionLabel,
  localFlatFoldabilityReasonLabel,
  type LocalFlatFoldabilityPresentation,
} from './localFlatFoldabilityPresentation'
import { APP_TEXT } from './appText.ts'

export function lineKindLabel(kind: CreaseLine['kind'], locale: Locale) {
  const labels: Readonly<Record<CreaseLine['kind'], LocalizedText>> = {
    mountain: APP_TEXT.mountainFold,
    valley: APP_TEXT.valleyFold,
    auxiliary: APP_TEXT.auxiliaryLine,
    boundary: APP_TEXT.boundaryEdge,
    cut: APP_TEXT.cutLine,
  }
  return selectLocalizedText(locale, labels[kind])
}

export function normalizeFoldAngle(value: number) {
  if (!Number.isFinite(value)) return null
  return Math.min(180, Math.max(0, value))
}

export function formatBytes(bytes: number, locale: Locale) {
  if (!Number.isFinite(bytes) || bytes < 0) {
    return selectLocalizedText(locale, APP_TEXT.unknownSize)
  }
  if (bytes < 1_000) return `${bytes} B`
  if (bytes < 1_000_000) return `${(bytes / 1_000).toFixed(1)} KB`
  return `${(bytes / 1_000_000).toFixed(2)} MB`
}

export function toolLabel(tool: string, locale: Locale) {
  const labels: Readonly<Record<string, LocalizedText>> = {
    select: APP_TEXT.select,
    vertex: APP_TEXT.vertex,
    mountain: APP_TEXT.mountainFold,
    valley: APP_TEXT.valleyFold,
    auxiliary: APP_TEXT.auxiliaryLine,
    cut: APP_TEXT.cut,
    measure: APP_TEXT.measure,
  }
  const label = labels[tool]
  return label
    ? selectLocalizedText(locale, label)
    : selectLocalizedText(locale, APP_TEXT.unknownTool)
}

export function validationIssueLabel(code: string, locale: Locale) {
  const labels: Readonly<Record<string, LocalizedText>> = {
    non_finite_vertex: APP_TEXT.nonFiniteVertexCoordinates,
    duplicate_vertex: APP_TEXT.duplicateVerticesAtTheSamePosition,
    missing_endpoint: APP_TEXT.lineReferencesAMissingEndpoint,
    zero_length_edge: APP_TEXT.zeroLengthLine,
    unsplit_intersection: APP_TEXT.unsplitIntersectionOrOverlap,
    intersection_calculation_failed: APP_TEXT.intersectionCalculationFailed,
    non_finite_thickness: APP_TEXT.paperThicknessIsNotFinite,
    negative_thickness: APP_TEXT.paperThicknessMustBeAtLeast0Mm,
    too_few_boundary_vertices: APP_TEXT.paperBoundaryNeedsAtLeastThreeVertices,
    duplicate_boundary_vertex: APP_TEXT.paperBoundaryContainsADuplicateVertex,
    missing_boundary_vertex: APP_TEXT.paperBoundaryReferencesAMissingVertex,
    non_finite_boundary_vertex: APP_TEXT.paperBoundaryVertexCoordinatesAreNotFinite,
    missing_boundary_edge: APP_TEXT.paperBoundaryEdgesAreMissing,
    duplicate_boundary_edge: APP_TEXT.paperBoundaryContainsADuplicateEdge,
    unexpected_boundary_edge: APP_TEXT.paperBoundaryContainsAnUnexpectedEdge,
    zero_length_boundary_edge: APP_TEXT.paperBoundaryContainsAZeroLengthEdge,
    boundary_self_intersection: APP_TEXT.paperBoundaryIntersectsItself,
    boundary_intersection_calculation_failed: APP_TEXT.paperBoundaryIntersectionTestFailed,
    zero_area_boundary: APP_TEXT.paperBoundaryHasZeroArea,
    boundary_area_calculation_failed: APP_TEXT.paperBoundaryAreaCalculationFailed,
  }
  const label = labels[code]
  return label
    ? selectLocalizedText(locale, label)
    : selectLocalizedText(locale, APP_TEXT.unknownGeometryValidationIssue)
}

export function localFlatFoldabilityCoreStatus(
  presentation: LocalFlatFoldabilityPresentation,
  locale: Locale,
) {
  if (presentation.kind === 'invalid') {
    return selectLocalizedText(locale, APP_TEXT.localResultUnavailable)
  }
  if (presentation.kind === 'blocked') {
    return selectLocalizedText(locale, APP_TEXT.localAnalysisBlockedByGeometryIssues)
  }
  if (presentation.reportStatus === 'necessary_conditions_satisfied') {
    return formatLocalizedText(locale, APP_TEXT.localNecessaryConditionsSatisfiedAtCountVertices, { count: presentation.counts.satisfied })
  }
  if (presentation.reportStatus === 'not_applicable') {
    return selectLocalizedText(locale, APP_TEXT.noVerticesEligibleForLocalAnalysis)
  }
  if (presentation.reportStatus === 'violated') {
    return formatLocalizedText(locale, APP_TEXT.localNecessaryConditionsViolatedAtCountVertices, { count: presentation.counts.violated })
  }
  return formatLocalizedText(locale, APP_TEXT.localResultIndeterminateAtCountVertices, { count: presentation.counts.indeterminate })
}

export function localizedLocalFlatFoldabilityConditionLabel(
  condition: Parameters<typeof localFlatFoldabilityConditionLabel>[0],
  locale: Locale,
) {
  if (locale === 'ja') return localFlatFoldabilityConditionLabel(condition)
  return {
    satisfied: 'Satisfied',
    violated: 'Violated',
    not_applicable: 'Not applicable',
    indeterminate: 'Indeterminate',
  }[condition]
}

export function localizedLocalFlatFoldabilityReasonLabel(
  reason: Parameters<typeof localFlatFoldabilityReasonLabel>[0],
  maxExactFoldDegree: number,
  locale: Locale,
) {
  if (locale === 'ja') {
    return localFlatFoldabilityReasonLabel(reason, maxExactFoldDegree)
  }
  switch (reason) {
    case 'paper_boundary':
      return 'Paper boundary vertices are outside the current local model.'
    case 'cut_incident':
      return 'Vertices incident to a cut line are outside the current local model.'
    case 'fold_degree_limit':
      return formatLocalizedText(locale, APP_TEXT.indeterminateBecauseTheFoldDegreeExceedsTheExactLimitLimit, { limit: maxExactFoldDegree })
    case 'no_incident_fold_edges':
      return 'Not applicable because there are no incident mountain or valley folds.'
    case null:
      return ''
  }
}

export function localizedLocalFlatFoldabilitySummary(
  presentation: LocalFlatFoldabilityPresentation,
  locale: Locale,
) {
  if (presentation.kind === 'invalid') {
    return selectLocalizedText(locale, APP_TEXT.theLocalFlatFoldabilityResultCouldNotBeVerifiedAnd)
  }
  if (presentation.kind === 'blocked') {
    return selectLocalizedText(locale, APP_TEXT.localFlatFoldabilityWasNotEvaluatedBecauseThePrecedingGeometry)
  }
  const detail = formatLocalizedText(locale, APP_TEXT.satisfiedSatisfiedViolatedViolatedNotApplicableNotApplicableIndeterminat, {
    satisfied: presentation.counts.satisfied,
    violated: presentation.counts.violated,
    notApplicable: presentation.counts.notApplicable,
    indeterminate: presentation.counts.indeterminate,
  })
  switch (presentation.reportStatus) {
    case 'necessary_conditions_satisfied':
      return formatLocalizedText(locale, APP_TEXT.localNecessaryConditionsAreSatisfiedWithinTheSupportedScopeDetail, { detail })
    case 'not_applicable':
      return formatLocalizedText(locale, APP_TEXT.noVerticesAreEligibleForTheCurrentLocalConditionsDetail, { detail })
    case 'violated':
      return formatLocalizedText(locale, APP_TEXT.someVerticesViolateTheLocalNecessaryConditionsDetail, { detail })
    case 'indeterminate':
      return formatLocalizedText(locale, APP_TEXT.someVerticesHaveIndeterminateLocalNecessaryConditionsDetail, { detail })
  }
}

export function localizedCreaseExportFormatLabel(
  format: CreasePatternExportFormat,
  locale: Locale,
) {
  if (locale === 'ja') return creasePatternExportFormatLabel(format)
  return format === 'dxf'
    ? 'DXF (AutoCAD 2007)'
    : creasePatternExportFormatLabel(format)
}

export function localizedInstructionExportFormatLabel(
  format: InstructionExportFormat,
  locale: Locale,
) {
  if (locale === 'ja') return instructionExportFormatLabel(format)
  return format === 'svg_zip'
    ? 'SVG images ZIP'
    : instructionExportFormatLabel(format)
}
