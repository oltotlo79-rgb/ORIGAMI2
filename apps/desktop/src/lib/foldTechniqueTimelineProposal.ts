import type {
  NamedTechniqueTimelineProposalStepV1,
  NamedTechniqueTimelineProposalV1,
  NamedTechniqueTimelineSourceKindV1,
} from './coreClient.ts'
import {
  type FoldTechniqueFileDocumentV1,
  type FoldTechniqueLocalizedTextV1,
  type FoldTechniqueOperationV1,
  type FoldTechniqueTemplateV1,
} from './foldTechniqueEditor.ts'
import type { Locale } from './i18n.ts'

export const NAMED_TECHNIQUE_TIMELINE_PROPOSAL_SCHEMA_VERSION_V1 = 1 as const
export const MAX_NAMED_TECHNIQUE_TIMELINE_PROPOSAL_BYTES = 2 * 1024 * 1024
export const MAX_NAMED_TECHNIQUE_TIMELINE_PROPOSAL_STEPS = 512

const MAX_DESCRIPTION_CHARACTERS = 4_000
const MAX_TITLE_CHARACTERS = 120
const DEFAULT_DURATION_MS = 1_500
const TEXT_ENCODER = new TextEncoder()

export type FoldTechniqueTimelineProposalError =
  | 'invalid_selection'
  | 'timeline_capacity'
  | 'proposal_size'

export type FoldTechniqueTimelineProposalPreview =
  | Readonly<{
      ok: true
      techniqueName: string
      operationCount: number
      unsupportedOperationCount: number
      proposal: NamedTechniqueTimelineProposalV1
    }>
  | Readonly<{
      ok: false
      error: FoldTechniqueTimelineProposalError
      requiredSteps: number
      availableSteps: number
    }>

type ProposalUnit = Readonly<{
  sourceKind: NamedTechniqueTimelineSourceKindV1
  sourceId: string
  title: string
  description: string
  caution: string
}>

/**
 * Converts one already-admitted named technique into an inert, deterministic
 * timeline proposal. Every source object is embedded as canonical JSON in the
 * descriptions, so localized text, parameter definitions, preconditions,
 * bindings, and execution-support declarations are retained without
 * truncation. Oversized units are split into consecutive description chunks.
 */
export function createFoldTechniqueTimelineProposalV1(
  document: FoldTechniqueFileDocumentV1,
  techniqueIndex: number,
  locale: Locale,
  existingStepCount: number,
): FoldTechniqueTimelineProposalPreview {
  const availableSteps = Number.isSafeInteger(existingStepCount)
    && existingStepCount >= 0
    && existingStepCount <= MAX_NAMED_TECHNIQUE_TIMELINE_PROPOSAL_STEPS
      ? MAX_NAMED_TECHNIQUE_TIMELINE_PROPOSAL_STEPS - existingStepCount
      : 0
  const technique = Number.isSafeInteger(techniqueIndex)
    && techniqueIndex >= 0
      ? document.techniques[techniqueIndex]
      : undefined
  if (!technique) {
    return Object.freeze({
      ok: false,
      error: 'invalid_selection',
      requiredSteps: 0,
      availableSteps,
    })
  }

  const units = proposalUnits(document, technique, locale)
  const steps = units.flatMap((unit) => splitUnit(unit))
  if (
    steps.length === 0
    || steps.length > MAX_NAMED_TECHNIQUE_TIMELINE_PROPOSAL_STEPS
    || steps.length > availableSteps
  ) {
    return Object.freeze({
      ok: false,
      error: 'timeline_capacity',
      requiredSteps: steps.length,
      availableSteps,
    })
  }

  const proposal = Object.freeze({
    schema_version: NAMED_TECHNIQUE_TIMELINE_PROPOSAL_SCHEMA_VERSION_V1,
    package_id: document.package_id,
    technique_id: technique.id,
    technique_version: technique.version,
    steps: Object.freeze(steps),
  }) satisfies NamedTechniqueTimelineProposalV1
  if (
    TEXT_ENCODER.encode(JSON.stringify(proposal)).length
      > MAX_NAMED_TECHNIQUE_TIMELINE_PROPOSAL_BYTES
  ) {
    return Object.freeze({
      ok: false,
      error: 'proposal_size',
      requiredSteps: steps.length,
      availableSteps,
    })
  }

  return Object.freeze({
    ok: true,
    techniqueName: localizedText(technique.names, locale, technique.id),
    operationCount: technique.operations.length,
    unsupportedOperationCount: technique.operations.filter(
      ({ execution_support: support }) =>
        support.status === 'unsupported_physical_operation',
    ).length,
    proposal,
  })
}

function proposalUnits(
  document: FoldTechniqueFileDocumentV1,
  technique: FoldTechniqueTemplateV1,
  locale: Locale,
): readonly ProposalUnit[] {
  const techniqueName = localizedText(technique.names, locale, technique.id)
  const units: ProposalUnit[] = [{
    sourceKind: 'technique',
    sourceId: technique.id,
    title: localized(locale, `技法: ${techniqueName}`, `Technique: ${techniqueName}`),
    description: sourceDescription(
      locale,
      '技法・出典情報',
      'Technique and provenance',
      {
        schema: 'origami2_named_technique_timeline_source_v1',
        package_id: document.package_id,
        metadata: document.metadata,
        technique: {
          id: technique.id,
          version: technique.version,
          names: technique.names,
          descriptions: technique.descriptions,
        },
      },
    ),
    caution: localized(
      locale,
      '説明専用の案です。3D姿勢や折り操作は実行しません。',
      'This is a description-only proposal. It does not apply a 3D pose or execute a fold.',
    ),
  }]

  for (const parameter of technique.parameters) {
    const name = localizedText(parameter.names, locale, parameter.id)
    units.push({
      sourceKind: 'parameter',
      sourceId: parameter.id,
      title: localized(locale, `設定: ${name}`, `Parameter: ${name}`),
      description: sourceDescription(
        locale,
        '設定値の定義',
        'Parameter definition',
        parameter,
      ),
      caution: '',
    })
  }
  for (const precondition of technique.preconditions) {
    units.push({
      sourceKind: 'precondition',
      sourceId: precondition.id,
      title: localized(
        locale,
        `前提条件: ${precondition.id}`,
        `Precondition: ${precondition.id}`,
      ),
      description: sourceDescription(
        locale,
        '実行前に確認する条件',
        'Condition to check before folding',
        precondition,
      ),
      caution: localized(
        locale,
        'この条件は自動判定しません。折り手が内容を確認してください。',
        'This condition is not evaluated automatically. The folder must verify it.',
      ),
    })
  }
  technique.operations.forEach((operation, index) => {
    const operationName = localizedText(
      operation.names,
      locale,
      operation.id,
    )
    units.push({
      sourceKind: 'operation',
      sourceId: operation.id,
      title: localized(
        locale,
        `操作 ${index + 1}: ${operationName}`,
        `Operation ${index + 1}: ${operationName}`,
      ),
      description: sourceDescription(
        locale,
        operationSummary(operation, locale),
        operationSummary(operation, locale),
        operation,
      ),
      caution: operationCaution(operation, locale),
    })
  })
  return Object.freeze(units)
}

function sourceDescription(
  locale: Locale,
  japaneseHeading: string,
  englishHeading: string,
  source: unknown,
) {
  return [
    localized(locale, japaneseHeading, englishHeading),
    'source-json-v1:',
    JSON.stringify(source),
  ].join('\n')
}

function splitUnit(unit: ProposalUnit): NamedTechniqueTimelineProposalStepV1[] {
  const characters = [...unit.description]
  const chunkCount = Math.max(
    1,
    Math.ceil(characters.length / MAX_DESCRIPTION_CHARACTERS),
  )
  const steps: NamedTechniqueTimelineProposalStepV1[] = []
  for (let index = 0; index < chunkCount; index += 1) {
    const description = characters
      .slice(
        index * MAX_DESCRIPTION_CHARACTERS,
        (index + 1) * MAX_DESCRIPTION_CHARACTERS,
      )
      .join('')
    steps.push(Object.freeze({
      source_kind: unit.sourceKind,
      source_id: unit.sourceId,
      chunk_index: index + 1,
      chunk_count: chunkCount,
      title: boundedTitle(unit.title, index + 1, chunkCount),
      description,
      caution: unit.caution,
      duration_ms: DEFAULT_DURATION_MS,
    }))
  }
  return steps
}

function boundedTitle(base: string, chunkIndex: number, chunkCount: number) {
  const suffix = chunkCount > 1 ? ` (${chunkIndex}/${chunkCount})` : ''
  const maximumBaseCharacters = MAX_TITLE_CHARACTERS - [...suffix].length
  const trimmedBase = [...base.trim()]
    .slice(0, Math.max(1, maximumBaseCharacters))
    .join('')
  return `${trimmedBase}${suffix}`
}

function operationSummary(
  operation: FoldTechniqueOperationV1,
  locale: Locale,
) {
  switch (operation.action.kind) {
    case 'instruction_cue':
      return localizedText(
        operation.action.instructions,
        locale,
        localized(locale, '文章による折り指示', 'Written folding cue'),
      )
    case 'layer_selective_manipulation':
      return localizedText(
        operation.action.instructions,
        locale,
        localized(locale, '層を選ぶ操作の説明', 'Layer-selective instruction'),
      )
    case 'straight_line_stacked_fold':
      return localized(locale, '一直線の折り重ね', 'Straight-line stacked fold')
    case 'inside_reverse_fold':
      return localized(locale, '中割り折り', 'Inside reverse fold')
    case 'outside_reverse_fold':
      return localized(locale, 'かぶせ折り', 'Outside reverse fold')
    case 'sink_fold':
      return operation.action.sink_kind === 'open'
        ? localized(locale, '開いた沈め折り', 'Open sink fold')
        : localized(locale, '閉じた沈め折り', 'Closed sink fold')
  }
}

function operationCaution(
  operation: FoldTechniqueOperationV1,
  locale: Locale,
) {
  if (operation.execution_support.status === 'unsupported_physical_operation') {
    return localized(
      locale,
      `未対応の物理操作（${operation.execution_support.operation}）です。説明テンプレートとしてのみ追加し、自動実行しません。`,
      `Unsupported physical operation (${operation.execution_support.operation}). It is added only as an explanation template and is never auto-executed.`,
    )
  }
  if (operation.action.kind === 'straight_line_stacked_fold') {
    return localized(
      locale,
      '折り重ね物理コマンドは実行しません。層・折り線を確認してから別途操作してください。',
      'No stacked-fold physical command is executed. Verify the layers and fold line before performing it separately.',
    )
  }
  return localized(
    locale,
    '説明専用ステップです。3D姿勢は変更しません。',
    'This is a description-only step. It does not change the 3D pose.',
  )
}

function localizedText(
  entries: readonly FoldTechniqueLocalizedTextV1[],
  locale: Locale,
  fallback: string,
) {
  return entries.find((entry) => entry.locale === locale)?.text
    ?? entries.find((entry) => entry.locale === 'ja')?.text
    ?? entries.find((entry) => entry.locale === 'en')?.text
    ?? entries[0]?.text
    ?? fallback
}

function localized(locale: Locale, japanese: string, english: string) {
  return locale === 'ja' ? japanese : english
}
