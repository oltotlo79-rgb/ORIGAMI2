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
import {
  FOLD_TECHNIQUE_TIMELINE_PROPOSAL_TEXT as TEXT,
} from './foldTechniqueTimelineProposalText.ts'
import {
  DEFAULT_LOCALE,
  formatLocalizedText,
  isLocale,
  selectLocalizedText,
  type Locale,
} from './i18n.ts'

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
    title: formatLocalizedText(
      locale,
      TEXT.techniqueTitle,
      { name: techniqueName },
    ),
    description: sourceDescription(
      selectLocalizedText(locale, TEXT.techniqueAndProvenance),
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
    caution: selectLocalizedText(
      locale,
      TEXT.descriptionOnlyProposal,
    ),
  }]

  for (const parameter of technique.parameters) {
    const name = localizedText(parameter.names, locale, parameter.id)
    units.push({
      sourceKind: 'parameter',
      sourceId: parameter.id,
      title: formatLocalizedText(
        locale,
        TEXT.parameterTitle,
        { name },
      ),
      description: sourceDescription(
        selectLocalizedText(locale, TEXT.parameterDefinition),
        parameter,
      ),
      caution: '',
    })
  }
  for (const precondition of technique.preconditions) {
    units.push({
      sourceKind: 'precondition',
      sourceId: precondition.id,
      title: formatLocalizedText(
        locale,
        TEXT.preconditionTitle,
        { id: precondition.id },
      ),
      description: sourceDescription(
        selectLocalizedText(locale, TEXT.preconditionCondition),
        precondition,
      ),
      caution: selectLocalizedText(
        locale,
        TEXT.preconditionCaution,
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
      title: formatLocalizedText(
        locale,
        TEXT.operationTitle,
        { index: index + 1, name: operationName },
      ),
      description: sourceDescription(
        operationSummary(operation, locale),
        operation,
      ),
      caution: operationCaution(operation, locale),
    })
  })
  return Object.freeze(units)
}

function sourceDescription(
  heading: string,
  source: unknown,
) {
  return [
    heading,
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
        selectLocalizedText(locale, TEXT.writtenFoldingCue),
      )
    case 'layer_selective_manipulation':
      return localizedText(
        operation.action.instructions,
        locale,
        selectLocalizedText(locale, TEXT.layerSelectiveInstruction),
      )
    case 'straight_line_stacked_fold':
      return selectLocalizedText(locale, TEXT.straightLineStackedFold)
    case 'inside_reverse_fold':
      return selectLocalizedText(locale, TEXT.insideReverseFold)
    case 'outside_reverse_fold':
      return selectLocalizedText(locale, TEXT.outsideReverseFold)
    case 'sink_fold':
      return operation.action.sink_kind === 'open'
        ? selectLocalizedText(locale, TEXT.openSinkFold)
        : selectLocalizedText(locale, TEXT.closedSinkFold)
  }
}

function operationCaution(
  operation: FoldTechniqueOperationV1,
  locale: Locale,
) {
  if (operation.execution_support.status === 'unsupported_physical_operation') {
    return formatLocalizedText(
      locale,
      TEXT.unsupportedPhysicalOperation,
      { operation: operation.execution_support.operation },
    )
  }
  if (operation.action.kind === 'straight_line_stacked_fold') {
    return selectLocalizedText(
      locale,
      TEXT.stackedFoldNotExecuted,
    )
  }
  return selectLocalizedText(
    locale,
    TEXT.descriptionOnlyStep,
  )
}

function localizedText(
  entries: readonly FoldTechniqueLocalizedTextV1[],
  locale: Locale,
  fallback: string,
) {
  const supportedLocale = isLocale(locale) ? locale : DEFAULT_LOCALE
  return entries.find((entry) => entry.locale === supportedLocale)?.text
    ?? entries.find((entry) => entry.locale === 'ja')?.text
    ?? entries.find((entry) => entry.locale === 'en')?.text
    ?? entries[0]?.text
    ?? fallback
}
