export const FOLD_TECHNIQUE_FILE_SCHEMA_V1 =
  'origami2_fold_technique_file' as const
export const FOLD_TECHNIQUE_FILE_VERSION_V1 = 1 as const

export const FOLD_TECHNIQUE_LIMITS_V1 = Object.freeze({
  encodedBytes: 1024 * 1024,
  packageIdBytes: 96,
  techniques: 64,
  authors: 8,
  authorChars: 120,
  authorBytes: 480,
  citationChars: 1_024,
  citationBytes: 4_096,
  licenseIdBytes: 64,
  identifierBytes: 96,
  locales: 8,
  localeBytes: 35,
  nameChars: 120,
  nameBytes: 480,
  descriptionChars: 2_048,
  descriptionBytes: 8_192,
  parameters: 64,
  choices: 32,
  preconditions: 128,
  preconditionDepth: 8,
  preconditionNodes: 512,
  operations: 256,
  operationBindings: 32,
  operationPreconditions: 32,
  operationCapabilities: 8,
  techniqueVersion: 1_000_000,
  lengthMicrometres: 10_000_000_000,
  absoluteAngleMicrodegrees: 180_000_000,
  ratioMillionths: 1_000_000_000,
  absoluteInteger: 1_000_000_000,
} as const)

export type FoldTechniqueLocalizedTextV1 = Readonly<{
  locale: string
  text: string
}>

export type FoldTechniqueSourceV1 =
  | Readonly<{ kind: 'user_authored' }>
  | Readonly<{ kind: 'adapted'; citation_text: string }>
  | Readonly<{ kind: 'published_reference'; citation_text: string }>

export type FoldTechniqueMetadataV1 = Readonly<{
  authors: readonly string[]
  source: FoldTechniqueSourceV1
  license_spdx_id: string
}>

export type FoldTechniqueChoiceOptionV1 = Readonly<{
  id: string
  names: readonly FoldTechniqueLocalizedTextV1[]
}>

export type FoldTechniqueParameterTypeV1 =
  | Readonly<{
      type: 'length_micrometres'
      minimum: number
      maximum: number
      default: number
    }>
  | Readonly<{
      type: 'angle_microdegrees'
      minimum: number
      maximum: number
      default: number
    }>
  | Readonly<{
      type: 'ratio_millionths'
      minimum: number
      maximum: number
      default: number
    }>
  | Readonly<{
      type: 'integer'
      minimum: number
      maximum: number
      default: number
    }>
  | Readonly<{ type: 'boolean'; default: boolean }>
  | Readonly<{
      type: 'choice'
      options: readonly FoldTechniqueChoiceOptionV1[]
      default_option_id: string
    }>

export type FoldTechniqueParameterDefinitionV1 = Readonly<{
  id: string
  names: readonly FoldTechniqueLocalizedTextV1[]
  descriptions: readonly FoldTechniqueLocalizedTextV1[]
  parameter_type: FoldTechniqueParameterTypeV1
}>

export type FoldTechniqueCapabilityV1 =
  | 'human_interpretation_v1'
  | 'instruction_timeline_v1'
  | 'manual_pose_registration_v1'
  | 'straight_line_stacked_fold_v1'
  | 'layer_selective_motion_v1'
  | 'inside_reverse_fold_motion_v1'
  | 'outside_reverse_fold_motion_v1'
  | 'sink_fold_motion_v1'

export type FoldTechniqueComparisonV1 =
  | 'equal'
  | 'not_equal'
  | 'less_than'
  | 'less_than_or_equal'
  | 'greater_than'
  | 'greater_than_or_equal'

export type FoldTechniqueParameterLiteralV1 =
  | Readonly<{ type: 'length_micrometres'; value: number }>
  | Readonly<{ type: 'angle_microdegrees'; value: number }>
  | Readonly<{ type: 'ratio_millionths'; value: number }>
  | Readonly<{ type: 'integer'; value: number }>
  | Readonly<{ type: 'boolean'; value: boolean }>
  | Readonly<{ type: 'choice'; option_id: string }>

export type FoldTechniquePreconditionV1 =
  | Readonly<{
      kind: 'all'
      conditions: readonly FoldTechniquePreconditionV1[]
    }>
  | Readonly<{
      kind: 'any'
      conditions: readonly FoldTechniquePreconditionV1[]
    }>
  | Readonly<{ kind: 'not'; condition: FoldTechniquePreconditionV1 }>
  | Readonly<{
      kind: 'parameter_comparison'
      parameter_id: string
      comparison: FoldTechniqueComparisonV1
      value: FoldTechniqueParameterLiteralV1
    }>
  | Readonly<{
      kind: 'capability_available'
      capability: FoldTechniqueCapabilityV1
    }>
  | Readonly<{
      kind: 'user_confirmation'
      prompts: readonly FoldTechniqueLocalizedTextV1[]
    }>

export type FoldTechniquePreconditionDefinitionV1 = Readonly<{
  id: string
  condition: FoldTechniquePreconditionV1
}>

export type FoldTechniqueParameterBindingV1 = Readonly<{
  role: string
  parameter_id: string
}>

export type FoldTechniqueActionV1 =
  | Readonly<{
      kind: 'instruction_cue'
      instructions: readonly FoldTechniqueLocalizedTextV1[]
    }>
  | Readonly<{ kind: 'straight_line_stacked_fold' }>
  | Readonly<{ kind: 'inside_reverse_fold' }>
  | Readonly<{ kind: 'outside_reverse_fold' }>
  | Readonly<{ kind: 'sink_fold'; sink_kind: 'open' | 'closed' }>
  | Readonly<{
      kind: 'layer_selective_manipulation'
      instructions: readonly FoldTechniqueLocalizedTextV1[]
    }>

export type FoldTechniqueActionKindV1 = FoldTechniqueActionV1['kind']

export type FoldTechniqueUnsupportedPhysicalOperationV1 =
  | 'layer_selective_motion_v1'
  | 'inside_reverse_fold_motion_v1'
  | 'outside_reverse_fold_motion_v1'
  | 'sink_fold_motion_v1'

export type FoldTechniqueExecutionSupportV1 =
  | Readonly<{ status: 'declarative_only' }>
  | Readonly<{
      status: 'unsupported_physical_operation'
      operation: FoldTechniqueUnsupportedPhysicalOperationV1
    }>

export type FoldTechniqueOperationV1 = Readonly<{
  id: string
  names: readonly FoldTechniqueLocalizedTextV1[]
  action: FoldTechniqueActionV1
  parameter_bindings: readonly FoldTechniqueParameterBindingV1[]
  precondition_ids: readonly string[]
  required_capabilities: readonly FoldTechniqueCapabilityV1[]
  execution_support: FoldTechniqueExecutionSupportV1
}>

export type FoldTechniqueTemplateV1 = Readonly<{
  id: string
  version: number
  names: readonly FoldTechniqueLocalizedTextV1[]
  descriptions: readonly FoldTechniqueLocalizedTextV1[]
  parameters: readonly FoldTechniqueParameterDefinitionV1[]
  preconditions: readonly FoldTechniquePreconditionDefinitionV1[]
  operations: readonly FoldTechniqueOperationV1[]
}>

export type FoldTechniqueFileDocumentV1 = Readonly<{
  schema: typeof FOLD_TECHNIQUE_FILE_SCHEMA_V1
  version: typeof FOLD_TECHNIQUE_FILE_VERSION_V1
  package_id: string
  metadata: FoldTechniqueMetadataV1
  techniques: readonly FoldTechniqueTemplateV1[]
}>

export type FoldTechniqueValidationErrorV1 =
  | 'invalid_structure'
  | 'unsupported_schema'
  | 'unsupported_version'
  | 'resource_limit'
  | 'invalid_field'
  | 'duplicate_identifier'
  | 'missing_reference'
  | 'parameter_type_mismatch'
  | 'inconsistent_execution_support'
  | 'encoded_size_limit'

export type FoldTechniqueValidationResultV1 =
  | Readonly<{ ok: true; document: FoldTechniqueFileDocumentV1 }>
  | Readonly<{ ok: false; error: FoldTechniqueValidationErrorV1 }>

export type FoldTechniqueDraftUpdateV1 =
  | Readonly<{ kind: 'package_id'; value: string }>
  | Readonly<{ kind: 'authors'; value: readonly string[] }>
  | Readonly<{ kind: 'source'; value: FoldTechniqueSourceV1 }>
  | Readonly<{ kind: 'license_spdx_id'; value: string }>
  | Readonly<{ kind: 'technique_id'; techniqueIndex: number; value: string }>
  | Readonly<{ kind: 'technique_version'; techniqueIndex: number; value: number }>
  | Readonly<{
      kind: 'technique_name' | 'technique_description'
      techniqueIndex: number
      locale: string
      value: string
    }>
  | Readonly<{
      kind: 'operation_id'
      techniqueIndex: number
      operationIndex: number
      value: string
    }>
  | Readonly<{
      kind: 'operation_name' | 'operation_instruction'
      techniqueIndex: number
      operationIndex: number
      locale: string
      value: string
    }>
  | Readonly<{
      kind: 'operation_action'
      techniqueIndex: number
      operationIndex: number
      value: FoldTechniqueActionKindV1
    }>
  | Readonly<{
      kind: 'operation_sink_kind'
      techniqueIndex: number
      operationIndex: number
      value: 'open' | 'closed'
    }>
  | Readonly<{
      kind: 'insert_operation'
      techniqueIndex: number
      operationIndex: number
      operation: FoldTechniqueOperationV1
    }>
  | Readonly<{
      kind: 'remove_operation'
      techniqueIndex: number
      operationIndex: number
    }>
  | Readonly<{
      kind: 'move_operation'
      techniqueIndex: number
      operationIndex: number
      direction: -1 | 1
    }>

const CAPABILITIES: readonly FoldTechniqueCapabilityV1[] = Object.freeze([
  'human_interpretation_v1',
  'instruction_timeline_v1',
  'manual_pose_registration_v1',
  'straight_line_stacked_fold_v1',
  'layer_selective_motion_v1',
  'inside_reverse_fold_motion_v1',
  'outside_reverse_fold_motion_v1',
  'sink_fold_motion_v1',
])

const COMPARISONS: readonly FoldTechniqueComparisonV1[] = Object.freeze([
  'equal',
  'not_equal',
  'less_than',
  'less_than_or_equal',
  'greater_than',
  'greater_than_or_equal',
])

const ACTION_KINDS: readonly FoldTechniqueActionKindV1[] = Object.freeze([
  'instruction_cue',
  'straight_line_stacked_fold',
  'inside_reverse_fold',
  'outside_reverse_fold',
  'sink_fold',
  'layer_selective_manipulation',
])

const CAPABILITY_ORDER = new Map(
  CAPABILITIES.map((capability, index) => [capability, index] as const),
)
const TEXT_ENCODER = new TextEncoder()
const IDENTIFIER_PATTERN = /^[a-z](?:[a-z0-9]|[._-](?=[a-z0-9]))*$/u
const SPDX_IDENTIFIER_PATTERN = /^[A-Za-z0-9.+-]+$/u
const LOCALE_PATTERN =
  /^[a-z]{2,8}(?:-[a-z0-9]{1,8})*$/u

class AdmissionFailure {
  readonly code: FoldTechniqueValidationErrorV1

  constructor(code: FoldTechniqueValidationErrorV1) {
    this.code = code
  }
}

type AdmissionContext = {
  preconditionNodes: number
}

type ExactRecord = Record<string, unknown>

export function validateFoldTechniqueDocumentV1(
  value: unknown,
): FoldTechniqueValidationResultV1 {
  try {
    const document = parseDocument(value)
    const encoded = JSON.stringify(document)
    if (TEXT_ENCODER.encode(encoded).byteLength
      > FOLD_TECHNIQUE_LIMITS_V1.encodedBytes) {
      fail('encoded_size_limit')
    }
    return Object.freeze({ ok: true, document: freezeDeep(document) })
  } catch (error) {
    return Object.freeze({
      ok: false,
      error: error instanceof AdmissionFailure
        ? error.code
        : 'invalid_structure',
    })
  }
}

export function admitFoldTechniqueDocumentV1(
  value: unknown,
): FoldTechniqueFileDocumentV1 | null {
  const result = validateFoldTechniqueDocumentV1(value)
  return result.ok ? result.document : null
}

export function createInitialFoldTechniqueDocumentV1():
FoldTechniqueFileDocumentV1 {
  const candidate = {
    schema: FOLD_TECHNIQUE_FILE_SCHEMA_V1,
    version: FOLD_TECHNIQUE_FILE_VERSION_V1,
    package_id: 'user.local.techniques',
    metadata: {
      authors: ['Local author'],
      source: { kind: 'user_authored' },
      license_spdx_id: 'LicenseRef-Proprietary',
    },
    techniques: [
      {
        id: 'user.new-technique',
        version: 1,
        names: localized('新しい折り技法', 'New folding technique'),
        descriptions: localized(
          '手順を編集して技法を説明します。',
          'Edit the ordered steps to describe the technique.',
        ),
        parameters: [],
        preconditions: [],
        operations: [
          createInitialFoldTechniqueOperationV1(1),
          createInitialFoldTechniqueOperationV1(2),
        ],
      },
    ],
  }
  const admitted = admitFoldTechniqueDocumentV1(candidate)
  if (!admitted) throw new Error('built-in fold-technique template is invalid')
  return admitted
}

export function createInitialFoldTechniqueOperationV1(
  ordinal: number,
): FoldTechniqueOperationV1 {
  const safeOrdinal = Number.isSafeInteger(ordinal) && ordinal > 0
    ? ordinal
    : 1
  return freezeDeep({
    id: `step-${safeOrdinal}`,
    names: localized(`手順${safeOrdinal}`, `Step ${safeOrdinal}`),
    action: {
      kind: 'instruction_cue',
      instructions: localized(
        'この手順を文章で説明してください。',
        'Describe this step for the folder.',
      ),
    },
    parameter_bindings: [],
    precondition_ids: [],
    required_capabilities: ['human_interpretation_v1'],
    execution_support: { status: 'declarative_only' },
  })
}

export function updateFoldTechniqueDocumentDraftV1(
  document: FoldTechniqueFileDocumentV1,
  update: FoldTechniqueDraftUpdateV1,
): FoldTechniqueFileDocumentV1 {
  switch (update.kind) {
    case 'package_id':
      return updateRootValue(document, 'package_id', update.value)
    case 'authors': {
      if (sameStringArray(document.metadata.authors, update.value)) return document
      return freezeDeep({
        ...document,
        metadata: { ...document.metadata, authors: [...update.value] },
      })
    }
    case 'source':
      if (sameSource(document.metadata.source, update.value)) return document
      return freezeDeep({
        ...document,
        metadata: { ...document.metadata, source: { ...update.value } },
      })
    case 'license_spdx_id':
      if (document.metadata.license_spdx_id === update.value) return document
      return freezeDeep({
        ...document,
        metadata: { ...document.metadata, license_spdx_id: update.value },
      })
    case 'technique_id':
      return updateTechnique(document, update.techniqueIndex, (technique) =>
        technique.id === update.value
          ? technique
          : { ...technique, id: update.value })
    case 'technique_version':
      return updateTechnique(document, update.techniqueIndex, (technique) =>
        technique.version === update.value
          ? technique
          : { ...technique, version: update.value })
    case 'technique_name':
    case 'technique_description':
      return updateTechnique(document, update.techniqueIndex, (technique) => {
        const field = update.kind === 'technique_name'
          ? 'names'
          : 'descriptions'
        const next = updateLocalizedTextDraftV1(
          technique[field],
          update.locale,
          update.value,
        )
        return next === technique[field]
          ? technique
          : { ...technique, [field]: next }
      })
    case 'operation_id':
      return updateOperation(
        document,
        update.techniqueIndex,
        update.operationIndex,
        (operation) => operation.id === update.value
          ? operation
          : { ...operation, id: update.value },
      )
    case 'operation_name':
      return updateOperation(
        document,
        update.techniqueIndex,
        update.operationIndex,
        (operation) => {
          const names = updateLocalizedTextDraftV1(
            operation.names,
            update.locale,
            update.value,
          )
          return names === operation.names ? operation : { ...operation, names }
        },
      )
    case 'operation_instruction':
      return updateOperation(
        document,
        update.techniqueIndex,
        update.operationIndex,
        (operation) => {
          if (
            operation.action.kind !== 'instruction_cue'
            && operation.action.kind !== 'layer_selective_manipulation'
          ) return operation
          const instructions = updateLocalizedTextDraftV1(
            operation.action.instructions,
            update.locale,
            update.value,
          )
          if (instructions === operation.action.instructions) return operation
          return {
            ...operation,
            action: { ...operation.action, instructions },
          }
        },
      )
    case 'operation_action':
      return updateOperation(
        document,
        update.techniqueIndex,
        update.operationIndex,
        (operation) => changeFoldTechniqueOperationActionV1(
          operation,
          update.value,
        ),
      )
    case 'operation_sink_kind':
      return updateOperation(
        document,
        update.techniqueIndex,
        update.operationIndex,
        (operation) => operation.action.kind !== 'sink_fold'
          || operation.action.sink_kind === update.value
          ? operation
          : {
              ...operation,
              action: { kind: 'sink_fold', sink_kind: update.value },
            },
      )
    case 'insert_operation':
      return updateTechnique(document, update.techniqueIndex, (technique) => {
        if (
          technique.operations.length >= FOLD_TECHNIQUE_LIMITS_V1.operations
          || !Number.isSafeInteger(update.operationIndex)
          || update.operationIndex < 0
          || update.operationIndex > technique.operations.length
        ) return technique
        const operations = [...technique.operations]
        operations.splice(update.operationIndex, 0, update.operation)
        return { ...technique, operations }
      })
    case 'remove_operation':
      return updateTechnique(document, update.techniqueIndex, (technique) => {
        if (
          technique.operations.length <= 2
          || !validIndex(update.operationIndex, technique.operations)
        ) return technique
        return {
          ...technique,
          operations: technique.operations.filter(
            (_, index) => index !== update.operationIndex,
          ),
        }
      })
    case 'move_operation':
      return updateTechnique(document, update.techniqueIndex, (technique) => {
        const destination = update.operationIndex + update.direction
        if (
          !validIndex(update.operationIndex, technique.operations)
          || !validIndex(destination, technique.operations)
        ) return technique
        const operations = [...technique.operations]
        const current = operations[update.operationIndex]
        const other = operations[destination]
        if (!current || !other) return technique
        operations[update.operationIndex] = other
        operations[destination] = current
        return { ...technique, operations }
      })
  }
}

export function updateLocalizedTextDraftV1(
  entries: readonly FoldTechniqueLocalizedTextV1[],
  locale: string,
  value: string,
): readonly FoldTechniqueLocalizedTextV1[] {
  const index = entries.findIndex((entry) => entry.locale === locale)
  if (index >= 0 && entries[index]?.text === value) return entries
  if (index < 0 && entries.length >= FOLD_TECHNIQUE_LIMITS_V1.locales) {
    return entries
  }
  const next = entries.map((entry) => ({ ...entry }))
  if (index >= 0) {
    next[index] = { locale, text: value }
  } else {
    next.push({ locale, text: value })
  }
  next.sort((left, right) => compareUtf8(left.locale, right.locale))
  return freezeDeep(next)
}

export function changeFoldTechniqueOperationActionV1(
  operation: FoldTechniqueOperationV1,
  actionKind: FoldTechniqueActionKindV1,
): FoldTechniqueOperationV1 {
  if (operation.action.kind === actionKind) return operation
  const policy = actionPolicy(actionKind)
  return freezeDeep({
    ...operation,
    action: initialAction(actionKind),
    required_capabilities: [policy.requiredCapability],
    execution_support: policy.executionSupport,
  })
}

export function foldTechniqueDocumentsEqualV1(
  left: FoldTechniqueFileDocumentV1,
  right: FoldTechniqueFileDocumentV1,
): boolean {
  if (left === right) return true
  return JSON.stringify(left) === JSON.stringify(right)
}

export function foldTechniqueLocalizedTextV1(
  entries: readonly FoldTechniqueLocalizedTextV1[],
  locale: string,
): string {
  return entries.find((entry) => entry.locale === locale)?.text ?? ''
}

export function isFoldTechniqueActionKindV1(
  value: unknown,
): value is FoldTechniqueActionKindV1 {
  return typeof value === 'string'
    && (ACTION_KINDS as readonly string[]).includes(value)
}

function parseDocument(value: unknown): FoldTechniqueFileDocumentV1 {
  const record = exactRecord(value, [
    'schema',
    'version',
    'package_id',
    'metadata',
    'techniques',
  ])
  if (record.schema !== FOLD_TECHNIQUE_FILE_SCHEMA_V1) {
    fail('unsupported_schema')
  }
  if (record.version !== FOLD_TECHNIQUE_FILE_VERSION_V1) {
    fail('unsupported_version')
  }
  const packageId = identifier(record.package_id, FOLD_TECHNIQUE_LIMITS_V1.packageIdBytes)
  const metadata = parseMetadata(record.metadata)
  const techniquesValue = exactArray(
    record.techniques,
    FOLD_TECHNIQUE_LIMITS_V1.techniques,
  )
  if (
    techniquesValue.length === 0
    || techniquesValue.length > FOLD_TECHNIQUE_LIMITS_V1.techniques
  ) fail('resource_limit')
  const techniques = techniquesValue.map(parseTechnique)
  ensureUnique(techniques.map((technique) => technique.id))
  techniques.sort((left, right) => compareUtf8(left.id, right.id))
  return {
    schema: FOLD_TECHNIQUE_FILE_SCHEMA_V1,
    version: FOLD_TECHNIQUE_FILE_VERSION_V1,
    package_id: packageId,
    metadata,
    techniques,
  }
}

function parseMetadata(value: unknown): FoldTechniqueMetadataV1 {
  const record = exactRecord(value, ['authors', 'source', 'license_spdx_id'])
  const authorsValue = exactArray(
    record.authors,
    FOLD_TECHNIQUE_LIMITS_V1.authors,
  )
  if (
    authorsValue.length === 0
    || authorsValue.length > FOLD_TECHNIQUE_LIMITS_V1.authors
  ) fail('resource_limit')
  const authors = authorsValue.map((author) =>
    boundedText(
      author,
      FOLD_TECHNIQUE_LIMITS_V1.authorChars,
      FOLD_TECHNIQUE_LIMITS_V1.authorBytes,
    ))
  ensureUnique(authors)
  authors.sort(compareUtf8)
  const source = parseSource(record.source)
  const license = stringValue(
    record.license_spdx_id,
    FOLD_TECHNIQUE_LIMITS_V1.licenseIdBytes,
  )
  if (
    byteLength(license) === 0
    || byteLength(license) > FOLD_TECHNIQUE_LIMITS_V1.licenseIdBytes
    || !SPDX_IDENTIFIER_PATTERN.test(license)
  ) fail('invalid_field')
  return { authors, source, license_spdx_id: license }
}

function parseSource(value: unknown): FoldTechniqueSourceV1 {
  const tag = taggedRecord(value, 'kind')
  switch (tag.kind) {
    case 'user_authored':
      exactRecord(value, ['kind'])
      return { kind: 'user_authored' }
    case 'adapted':
    case 'published_reference': {
      const record = exactRecord(value, ['kind', 'citation_text'])
      return {
        kind: tag.kind,
        citation_text: boundedText(
          record.citation_text,
          FOLD_TECHNIQUE_LIMITS_V1.citationChars,
          FOLD_TECHNIQUE_LIMITS_V1.citationBytes,
        ),
      }
    }
    default:
      fail('invalid_structure')
  }
}

function parseTechnique(value: unknown): FoldTechniqueTemplateV1 {
  const record = exactRecord(value, [
    'id',
    'version',
    'names',
    'descriptions',
    'parameters',
    'preconditions',
    'operations',
  ])
  const id = identifier(record.id)
  const version = safeInteger(record.version)
  if (version < 1 || version > FOLD_TECHNIQUE_LIMITS_V1.techniqueVersion) {
    fail('invalid_field')
  }
  const names = parseLocalizedTexts(
    record.names,
    FOLD_TECHNIQUE_LIMITS_V1.nameChars,
    FOLD_TECHNIQUE_LIMITS_V1.nameBytes,
  )
  const descriptions = parseLocalizedTexts(
    record.descriptions,
    FOLD_TECHNIQUE_LIMITS_V1.descriptionChars,
    FOLD_TECHNIQUE_LIMITS_V1.descriptionBytes,
  )
  const parameterValues = exactArray(
    record.parameters,
    FOLD_TECHNIQUE_LIMITS_V1.parameters,
  )
  if (parameterValues.length > FOLD_TECHNIQUE_LIMITS_V1.parameters) {
    fail('resource_limit')
  }
  const parameters = parameterValues.map(parseParameter)
  ensureUnique(parameters.map((parameter) => parameter.id))
  parameters.sort((left, right) => compareUtf8(left.id, right.id))
  const parameterMap = new Map(
    parameters.map((parameter) => [parameter.id, parameter.parameter_type] as const),
  )

  const preconditionValues = exactArray(
    record.preconditions,
    FOLD_TECHNIQUE_LIMITS_V1.preconditions,
  )
  if (preconditionValues.length > FOLD_TECHNIQUE_LIMITS_V1.preconditions) {
    fail('resource_limit')
  }
  const context: AdmissionContext = { preconditionNodes: 0 }
  const preconditions = preconditionValues.map((precondition) =>
    parsePreconditionDefinition(precondition, parameterMap, context))
  ensureUnique(preconditions.map((precondition) => precondition.id))
  preconditions.sort((left, right) => compareUtf8(left.id, right.id))
  const preconditionIds = new Set(
    preconditions.map((precondition) => precondition.id),
  )

  const operationValues = exactArray(
    record.operations,
    FOLD_TECHNIQUE_LIMITS_V1.operations,
  )
  if (
    operationValues.length < 2
    || operationValues.length > FOLD_TECHNIQUE_LIMITS_V1.operations
  ) fail('resource_limit')
  const operations = operationValues.map((operation) =>
    parseOperation(operation, parameterMap, preconditionIds))
  ensureUnique(operations.map((operation) => operation.id))
  return {
    id,
    version,
    names,
    descriptions,
    parameters,
    preconditions,
    operations,
  }
}

function parseParameter(value: unknown): FoldTechniqueParameterDefinitionV1 {
  const record = exactRecord(value, [
    'id',
    'names',
    'descriptions',
    'parameter_type',
  ])
  return {
    id: identifier(record.id),
    names: parseLocalizedTexts(
      record.names,
      FOLD_TECHNIQUE_LIMITS_V1.nameChars,
      FOLD_TECHNIQUE_LIMITS_V1.nameBytes,
    ),
    descriptions: parseLocalizedTexts(
      record.descriptions,
      FOLD_TECHNIQUE_LIMITS_V1.descriptionChars,
      FOLD_TECHNIQUE_LIMITS_V1.descriptionBytes,
    ),
    parameter_type: parseParameterType(record.parameter_type),
  }
}

function parseParameterType(value: unknown): FoldTechniqueParameterTypeV1 {
  const tag = taggedRecord(value, 'type')
  switch (tag.type) {
    case 'length_micrometres':
      return parseNumericParameter(
        value,
        tag.type,
        0,
        FOLD_TECHNIQUE_LIMITS_V1.lengthMicrometres,
      )
    case 'angle_microdegrees':
      return parseNumericParameter(
        value,
        tag.type,
        -FOLD_TECHNIQUE_LIMITS_V1.absoluteAngleMicrodegrees,
        FOLD_TECHNIQUE_LIMITS_V1.absoluteAngleMicrodegrees,
      )
    case 'ratio_millionths':
      return parseNumericParameter(
        value,
        tag.type,
        1,
        FOLD_TECHNIQUE_LIMITS_V1.ratioMillionths,
      )
    case 'integer':
      return parseNumericParameter(
        value,
        tag.type,
        -FOLD_TECHNIQUE_LIMITS_V1.absoluteInteger,
        FOLD_TECHNIQUE_LIMITS_V1.absoluteInteger,
      )
    case 'boolean': {
      const record = exactRecord(value, ['type', 'default'])
      if (typeof record.default !== 'boolean') fail('invalid_field')
      return { type: 'boolean', default: record.default }
    }
    case 'choice': {
      const record = exactRecord(value, [
        'type',
        'options',
        'default_option_id',
      ])
      const values = exactArray(
        record.options,
        FOLD_TECHNIQUE_LIMITS_V1.choices,
      )
      if (
        values.length === 0
        || values.length > FOLD_TECHNIQUE_LIMITS_V1.choices
      ) fail('resource_limit')
      const options = values.map((option) => {
        const optionRecord = exactRecord(option, ['id', 'names'])
        return {
          id: identifier(optionRecord.id),
          names: parseLocalizedTexts(
            optionRecord.names,
            FOLD_TECHNIQUE_LIMITS_V1.nameChars,
            FOLD_TECHNIQUE_LIMITS_V1.nameBytes,
          ),
        }
      })
      ensureUnique(options.map((option) => option.id))
      const defaultOptionId = identifier(record.default_option_id)
      if (!options.some((option) => option.id === defaultOptionId)) {
        fail('missing_reference')
      }
      return {
        type: 'choice',
        options,
        default_option_id: defaultOptionId,
      }
    }
    default:
      fail('invalid_structure')
  }
}

function parseNumericParameter<
  Kind extends
  | 'length_micrometres'
  | 'angle_microdegrees'
  | 'ratio_millionths'
  | 'integer',
>(
  value: unknown,
  type: Kind,
  allowedMinimum: number,
  allowedMaximum: number,
): Readonly<{
  type: Kind
  minimum: number
  maximum: number
  default: number
}> {
  const record = exactRecord(value, [
    'type',
    'minimum',
    'maximum',
    'default',
  ])
  const minimum = safeInteger(record.minimum)
  const maximum = safeInteger(record.maximum)
  const defaultValue = safeInteger(record.default)
  if (
    minimum < allowedMinimum
    || maximum > allowedMaximum
    || minimum > maximum
    || defaultValue < minimum
    || defaultValue > maximum
  ) fail('invalid_field')
  return { type, minimum, maximum, default: defaultValue }
}

function parsePreconditionDefinition(
  value: unknown,
  parameters: ReadonlyMap<string, FoldTechniqueParameterTypeV1>,
  context: AdmissionContext,
): FoldTechniquePreconditionDefinitionV1 {
  const record = exactRecord(value, ['id', 'condition'])
  return {
    id: identifier(record.id),
    condition: parsePrecondition(record.condition, parameters, context, 1),
  }
}

function parsePrecondition(
  value: unknown,
  parameters: ReadonlyMap<string, FoldTechniqueParameterTypeV1>,
  context: AdmissionContext,
  depth: number,
): FoldTechniquePreconditionV1 {
  if (depth > FOLD_TECHNIQUE_LIMITS_V1.preconditionDepth) {
    fail('resource_limit')
  }
  context.preconditionNodes += 1
  if (context.preconditionNodes > FOLD_TECHNIQUE_LIMITS_V1.preconditionNodes) {
    fail('resource_limit')
  }
  const tag = taggedRecord(value, 'kind')
  switch (tag.kind) {
    case 'all':
    case 'any': {
      const record = exactRecord(value, ['kind', 'conditions'])
      const children = exactArray(
        record.conditions,
        FOLD_TECHNIQUE_LIMITS_V1.preconditionNodes,
      )
      if (
        children.length === 0
        || children.length > FOLD_TECHNIQUE_LIMITS_V1.preconditionNodes
      ) fail('resource_limit')
      return {
        kind: tag.kind,
        conditions: children.map((child) =>
          parsePrecondition(child, parameters, context, depth + 1)),
      }
    }
    case 'not': {
      const record = exactRecord(value, ['kind', 'condition'])
      return {
        kind: 'not',
        condition: parsePrecondition(
          record.condition,
          parameters,
          context,
          depth + 1,
        ),
      }
    }
    case 'parameter_comparison': {
      const record = exactRecord(value, [
        'kind',
        'parameter_id',
        'comparison',
        'value',
      ])
      const parameterId = identifier(record.parameter_id)
      const parameter = parameters.get(parameterId)
      if (!parameter) fail('missing_reference')
      const comparison = enumValue(record.comparison, COMPARISONS)
      const literal = parseLiteral(record.value, parameter, comparison)
      return {
        kind: 'parameter_comparison',
        parameter_id: parameterId,
        comparison,
        value: literal,
      }
    }
    case 'capability_available': {
      const record = exactRecord(value, ['kind', 'capability'])
      return {
        kind: 'capability_available',
        capability: enumValue(record.capability, CAPABILITIES),
      }
    }
    case 'user_confirmation': {
      const record = exactRecord(value, ['kind', 'prompts'])
      return {
        kind: 'user_confirmation',
        prompts: parseLocalizedTexts(
          record.prompts,
          FOLD_TECHNIQUE_LIMITS_V1.descriptionChars,
          FOLD_TECHNIQUE_LIMITS_V1.descriptionBytes,
        ),
      }
    }
    default:
      fail('invalid_structure')
  }
}

function parseLiteral(
  value: unknown,
  parameter: FoldTechniqueParameterTypeV1,
  comparison: FoldTechniqueComparisonV1,
): FoldTechniqueParameterLiteralV1 {
  const tag = taggedRecord(value, 'type')
  const equalityOnly = comparison === 'equal' || comparison === 'not_equal'
  switch (parameter.type) {
    case 'length_micrometres':
    case 'angle_microdegrees':
    case 'ratio_millionths':
    case 'integer': {
      if (tag.type !== parameter.type) fail('parameter_type_mismatch')
      const record = exactRecord(value, ['type', 'value'])
      const literalValue = safeInteger(record.value)
      if (
        literalValue < parameter.minimum
        || literalValue > parameter.maximum
      ) fail('parameter_type_mismatch')
      return { type: parameter.type, value: literalValue }
    }
    case 'boolean': {
      if (tag.type !== 'boolean' || !equalityOnly) {
        fail('parameter_type_mismatch')
      }
      const record = exactRecord(value, ['type', 'value'])
      if (typeof record.value !== 'boolean') fail('parameter_type_mismatch')
      return { type: 'boolean', value: record.value }
    }
    case 'choice': {
      if (tag.type !== 'choice' || !equalityOnly) {
        fail('parameter_type_mismatch')
      }
      const record = exactRecord(value, ['type', 'option_id'])
      const optionId = identifier(record.option_id)
      if (!parameter.options.some((option) => option.id === optionId)) {
        fail('parameter_type_mismatch')
      }
      return { type: 'choice', option_id: optionId }
    }
  }
}

function parseOperation(
  value: unknown,
  parameters: ReadonlyMap<string, FoldTechniqueParameterTypeV1>,
  preconditionIds: ReadonlySet<string>,
): FoldTechniqueOperationV1 {
  const record = exactRecord(value, [
    'id',
    'names',
    'action',
    'parameter_bindings',
    'precondition_ids',
    'required_capabilities',
    'execution_support',
  ])
  const bindingsValue = exactArray(
    record.parameter_bindings,
    FOLD_TECHNIQUE_LIMITS_V1.operationBindings,
  )
  if (bindingsValue.length > FOLD_TECHNIQUE_LIMITS_V1.operationBindings) {
    fail('resource_limit')
  }
  const parameterBindings = bindingsValue.map((binding) => {
    const bindingRecord = exactRecord(binding, ['role', 'parameter_id'])
    const role = identifier(bindingRecord.role)
    const parameterId = identifier(bindingRecord.parameter_id)
    if (!parameters.has(parameterId)) fail('missing_reference')
    return { role, parameter_id: parameterId }
  })
  ensureUnique(parameterBindings.map((binding) => binding.role))
  parameterBindings.sort((left, right) => compareUtf8(left.role, right.role))

  const referencesValue = exactArray(
    record.precondition_ids,
    FOLD_TECHNIQUE_LIMITS_V1.operationPreconditions,
  )
  if (referencesValue.length
    > FOLD_TECHNIQUE_LIMITS_V1.operationPreconditions) {
    fail('resource_limit')
  }
  const references = referencesValue.map((reference) => identifier(reference))
  ensureUnique(references)
  if (references.some((reference) => !preconditionIds.has(reference))) {
    fail('missing_reference')
  }
  references.sort(compareUtf8)

  const capabilitiesValue = exactArray(
    record.required_capabilities,
    FOLD_TECHNIQUE_LIMITS_V1.operationCapabilities,
  )
  if (
    capabilitiesValue.length === 0
    || capabilitiesValue.length
      > FOLD_TECHNIQUE_LIMITS_V1.operationCapabilities
  ) fail('resource_limit')
  const capabilities = capabilitiesValue.map((capability) =>
    enumValue(capability, CAPABILITIES))
  ensureUnique(capabilities)
  capabilities.sort(
    (left, right) =>
      (CAPABILITY_ORDER.get(left) ?? 0) - (CAPABILITY_ORDER.get(right) ?? 0),
  )
  const action = parseAction(record.action)
  const executionSupport = parseExecutionSupport(record.execution_support)
  validateExecutionSupport(action, capabilities, executionSupport)
  return {
    id: identifier(record.id),
    names: parseLocalizedTexts(
      record.names,
      FOLD_TECHNIQUE_LIMITS_V1.nameChars,
      FOLD_TECHNIQUE_LIMITS_V1.nameBytes,
    ),
    action,
    parameter_bindings: parameterBindings,
    precondition_ids: references,
    required_capabilities: capabilities,
    execution_support: executionSupport,
  }
}

function parseAction(value: unknown): FoldTechniqueActionV1 {
  const tag = taggedRecord(value, 'kind')
  switch (tag.kind) {
    case 'instruction_cue': {
      const record = exactRecord(value, ['kind', 'instructions'])
      return {
        kind: 'instruction_cue',
        instructions: parseLocalizedTexts(
          record.instructions,
          FOLD_TECHNIQUE_LIMITS_V1.descriptionChars,
          FOLD_TECHNIQUE_LIMITS_V1.descriptionBytes,
        ),
      }
    }
    case 'straight_line_stacked_fold':
    case 'inside_reverse_fold':
    case 'outside_reverse_fold':
      exactRecord(value, ['kind'])
      return { kind: tag.kind }
    case 'sink_fold': {
      const record = exactRecord(value, ['kind', 'sink_kind'])
      if (record.sink_kind !== 'open' && record.sink_kind !== 'closed') {
        fail('invalid_structure')
      }
      return { kind: 'sink_fold', sink_kind: record.sink_kind }
    }
    case 'layer_selective_manipulation': {
      const record = exactRecord(value, ['kind', 'instructions'])
      return {
        kind: 'layer_selective_manipulation',
        instructions: parseLocalizedTexts(
          record.instructions,
          FOLD_TECHNIQUE_LIMITS_V1.descriptionChars,
          FOLD_TECHNIQUE_LIMITS_V1.descriptionBytes,
        ),
      }
    }
    default:
      fail('invalid_structure')
  }
}

function parseExecutionSupport(
  value: unknown,
): FoldTechniqueExecutionSupportV1 {
  const tag = taggedRecord(value, 'status')
  if (tag.status === 'declarative_only') {
    exactRecord(value, ['status'])
    return { status: 'declarative_only' }
  }
  if (tag.status === 'unsupported_physical_operation') {
    const record = exactRecord(value, ['status', 'operation'])
    if (
      record.operation !== 'layer_selective_motion_v1'
      && record.operation !== 'inside_reverse_fold_motion_v1'
      && record.operation !== 'outside_reverse_fold_motion_v1'
      && record.operation !== 'sink_fold_motion_v1'
    ) fail('invalid_structure')
    return {
      status: 'unsupported_physical_operation',
      operation: record.operation,
    }
  }
  fail('invalid_structure')
}

function validateExecutionSupport(
  action: FoldTechniqueActionV1,
  capabilities: readonly FoldTechniqueCapabilityV1[],
  support: FoldTechniqueExecutionSupportV1,
) {
  const policy = actionPolicy(action.kind)
  const supportMatches = support.status === policy.executionSupport.status
    && (
      support.status === 'declarative_only'
      || (
        policy.executionSupport.status === 'unsupported_physical_operation'
        && support.operation === policy.executionSupport.operation
      )
    )
  if (
    !supportMatches
    || !capabilities.includes(policy.requiredCapability)
    || capabilities.some((capability) =>
      isUnsupportedPhysicalCapability(capability)
      && capability !== policy.requiredCapability)
  ) fail('inconsistent_execution_support')
}

function parseLocalizedTexts(
  value: unknown,
  maximumChars: number,
  maximumBytes: number,
): FoldTechniqueLocalizedTextV1[] {
  const values = exactArray(value, FOLD_TECHNIQUE_LIMITS_V1.locales)
  if (
    values.length === 0
    || values.length > FOLD_TECHNIQUE_LIMITS_V1.locales
  ) fail('resource_limit')
  const entries = values.map((entry) => {
    const record = exactRecord(entry, ['locale', 'text'])
    const locale = stringValue(
      record.locale,
      FOLD_TECHNIQUE_LIMITS_V1.localeBytes,
    )
    if (
      byteLength(locale) === 0
      || byteLength(locale) > FOLD_TECHNIQUE_LIMITS_V1.localeBytes
      || !LOCALE_PATTERN.test(locale)
    ) fail('invalid_field')
    return {
      locale,
      text: boundedText(record.text, maximumChars, maximumBytes),
    }
  })
  ensureUnique(entries.map((entry) => entry.locale))
  entries.sort((left, right) => compareUtf8(left.locale, right.locale))
  return entries
}

function actionPolicy(actionKind: FoldTechniqueActionKindV1): Readonly<{
  requiredCapability: FoldTechniqueCapabilityV1
  executionSupport: FoldTechniqueExecutionSupportV1
}> {
  switch (actionKind) {
    case 'instruction_cue':
      return {
        requiredCapability: 'human_interpretation_v1',
        executionSupport: { status: 'declarative_only' },
      }
    case 'straight_line_stacked_fold':
      return {
        requiredCapability: 'straight_line_stacked_fold_v1',
        executionSupport: { status: 'declarative_only' },
      }
    case 'inside_reverse_fold':
      return {
        requiredCapability: 'inside_reverse_fold_motion_v1',
        executionSupport: {
          status: 'unsupported_physical_operation',
          operation: 'inside_reverse_fold_motion_v1',
        },
      }
    case 'outside_reverse_fold':
      return {
        requiredCapability: 'outside_reverse_fold_motion_v1',
        executionSupport: {
          status: 'unsupported_physical_operation',
          operation: 'outside_reverse_fold_motion_v1',
        },
      }
    case 'sink_fold':
      return {
        requiredCapability: 'sink_fold_motion_v1',
        executionSupport: {
          status: 'unsupported_physical_operation',
          operation: 'sink_fold_motion_v1',
        },
      }
    case 'layer_selective_manipulation':
      return {
        requiredCapability: 'layer_selective_motion_v1',
        executionSupport: {
          status: 'unsupported_physical_operation',
          operation: 'layer_selective_motion_v1',
        },
      }
  }
}

function initialAction(actionKind: FoldTechniqueActionKindV1):
FoldTechniqueActionV1 {
  switch (actionKind) {
    case 'instruction_cue':
      return {
        kind: actionKind,
        instructions: localized(
          'この手順を文章で説明してください。',
          'Describe this step for the folder.',
        ),
      }
    case 'layer_selective_manipulation':
      return {
        kind: actionKind,
        instructions: localized(
          '層を選ぶ操作を文章で説明してください。',
          'Describe the layer-selective motion for the folder.',
        ),
      }
    case 'sink_fold':
      return { kind: actionKind, sink_kind: 'open' }
    case 'straight_line_stacked_fold':
    case 'inside_reverse_fold':
    case 'outside_reverse_fold':
      return { kind: actionKind }
  }
}

function updateRootValue<
  Key extends 'package_id',
>(
  document: FoldTechniqueFileDocumentV1,
  key: Key,
  value: FoldTechniqueFileDocumentV1[Key],
): FoldTechniqueFileDocumentV1 {
  if (document[key] === value) return document
  return freezeDeep({ ...document, [key]: value })
}

function updateTechnique(
  document: FoldTechniqueFileDocumentV1,
  techniqueIndex: number,
  updater: (
    technique: FoldTechniqueTemplateV1,
  ) => FoldTechniqueTemplateV1,
): FoldTechniqueFileDocumentV1 {
  if (!validIndex(techniqueIndex, document.techniques)) return document
  const current = document.techniques[techniqueIndex]
  if (!current) return document
  const next = updater(current)
  if (next === current) return document
  const techniques = [...document.techniques]
  techniques[techniqueIndex] = next
  return freezeDeep({ ...document, techniques })
}

function updateOperation(
  document: FoldTechniqueFileDocumentV1,
  techniqueIndex: number,
  operationIndex: number,
  updater: (
    operation: FoldTechniqueOperationV1,
  ) => FoldTechniqueOperationV1,
): FoldTechniqueFileDocumentV1 {
  return updateTechnique(document, techniqueIndex, (technique) => {
    if (!validIndex(operationIndex, technique.operations)) return technique
    const current = technique.operations[operationIndex]
    if (!current) return technique
    const next = updater(current)
    if (next === current) return technique
    const operations = [...technique.operations]
    operations[operationIndex] = next
    return { ...technique, operations }
  })
}

function localized(
  ja: string,
  en: string,
): readonly FoldTechniqueLocalizedTextV1[] {
  return [
    { locale: 'ja', text: ja },
    { locale: 'en', text: en },
  ]
}

function exactRecord(
  value: unknown,
  keys: readonly string[],
): ExactRecord {
  try {
    if (typeof value !== 'object' || value === null || Array.isArray(value)) {
      fail('invalid_structure')
    }
    const prototype = Object.getPrototypeOf(value)
    if (prototype !== Object.prototype && prototype !== null) {
      fail('invalid_structure')
    }
    const descriptors = Object.getOwnPropertyDescriptors(value)
    const ownKeys = Reflect.ownKeys(descriptors)
    if (
      ownKeys.length !== keys.length
      || ownKeys.some((key) =>
        typeof key !== 'string' || !keys.includes(key))
    ) fail('invalid_structure')
    const snapshot: ExactRecord = Object.create(null)
    for (const key of keys) {
      const descriptor = descriptors[key]
      if (
        !descriptor
        || !('value' in descriptor)
        || !descriptor.enumerable
      ) fail('invalid_structure')
      snapshot[key] = descriptor.value
    }
    return snapshot
  } catch (error) {
    if (error instanceof AdmissionFailure) throw error
    fail('invalid_structure')
  }
}

function taggedRecord(value: unknown, tag: string): ExactRecord {
  try {
    if (typeof value !== 'object' || value === null || Array.isArray(value)) {
      fail('invalid_structure')
    }
    const prototype = Object.getPrototypeOf(value)
    if (prototype !== Object.prototype && prototype !== null) {
      fail('invalid_structure')
    }
    const descriptor = Object.getOwnPropertyDescriptor(value, tag)
    if (!descriptor || !('value' in descriptor) || !descriptor.enumerable) {
      fail('invalid_structure')
    }
    return { [tag]: descriptor.value }
  } catch (error) {
    if (error instanceof AdmissionFailure) throw error
    fail('invalid_structure')
  }
}

function exactArray(value: unknown, maximumLength: number): unknown[] {
  try {
    if (!Array.isArray(value) || Object.getPrototypeOf(value) !== Array.prototype) {
      fail('invalid_structure')
    }
    const lengthDescriptor = Object.getOwnPropertyDescriptor(value, 'length')
    const length = lengthDescriptor && 'value' in lengthDescriptor
      ? lengthDescriptor.value as unknown
      : null
    if (
      !lengthDescriptor
      || !('value' in lengthDescriptor)
      || typeof length !== 'number'
      || !Number.isSafeInteger(length)
      || length < 0
    ) fail('invalid_structure')
    if (length > maximumLength) fail('resource_limit')
    const descriptors = Object.getOwnPropertyDescriptors(value) as unknown as
      Record<PropertyKey, PropertyDescriptor | undefined>
    const ownKeys = Reflect.ownKeys(descriptors)
    if (ownKeys.length !== length + 1) fail('invalid_structure')
    const result: unknown[] = []
    for (let index = 0; index < length; index += 1) {
      const descriptor = descriptors[String(index)]
      if (
        !descriptor
        || !('value' in descriptor)
        || !descriptor.enumerable
      ) fail('invalid_structure')
      result.push(descriptor.value)
    }
    if (ownKeys.some((key) =>
      key !== 'length'
      && (
        typeof key !== 'string'
        || !/^(?:0|[1-9][0-9]*)$/u.test(key)
      ))) fail('invalid_structure')
    return result
  } catch (error) {
    if (error instanceof AdmissionFailure) throw error
    fail('invalid_structure')
  }
}

function stringValue(value: unknown, maximumCodeUnits: number): string {
  if (
    typeof value !== 'string'
    || value.length > maximumCodeUnits
    || hasLoneSurrogate(value)
  ) {
    fail('invalid_field')
  }
  return value
}

function boundedText(
  value: unknown,
  maximumChars: number,
  maximumBytes: number,
): string {
  const text = stringValue(value, maximumBytes)
  if (
    text.length === 0
    || text.trim() !== text
    || [...text].length > maximumChars
    || byteLength(text) > maximumBytes
    || [...text].some(isDisallowedTextCharacter)
  ) fail('invalid_field')
  return text
}

function identifier(
  value: unknown,
  maximumBytes = FOLD_TECHNIQUE_LIMITS_V1.identifierBytes,
): string {
  const result = stringValue(value, maximumBytes)
  if (
    byteLength(result) === 0
    || byteLength(result) > maximumBytes
    || !IDENTIFIER_PATTERN.test(result)
  ) fail('invalid_field')
  return result
}

function safeInteger(value: unknown): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value)) {
    fail('invalid_field')
  }
  return value
}

function enumValue<const Value extends string>(
  value: unknown,
  allowed: readonly Value[],
): Value {
  if (
    typeof value !== 'string'
    || !(allowed as readonly string[]).includes(value)
  ) fail('invalid_structure')
  return value as Value
}

function ensureUnique(values: readonly string[]) {
  if (new Set(values).size !== values.length) fail('duplicate_identifier')
}

function sameStringArray(
  left: readonly string[],
  right: readonly string[],
) {
  return left.length === right.length
    && left.every((value, index) => value === right[index])
}

function sameSource(
  left: FoldTechniqueSourceV1,
  right: FoldTechniqueSourceV1,
) {
  if (left.kind !== right.kind) return false
  if (left.kind === 'user_authored' || right.kind === 'user_authored') {
    return true
  }
  return left.citation_text === right.citation_text
}

function validIndex(index: number, values: readonly unknown[]) {
  return Number.isSafeInteger(index) && index >= 0 && index < values.length
}

function isUnsupportedPhysicalCapability(
  capability: FoldTechniqueCapabilityV1,
): capability is FoldTechniqueUnsupportedPhysicalOperationV1 {
  return capability === 'layer_selective_motion_v1'
    || capability === 'inside_reverse_fold_motion_v1'
    || capability === 'outside_reverse_fold_motion_v1'
    || capability === 'sink_fold_motion_v1'
}

function isDisallowedTextCharacter(character: string) {
  const codePoint = character.codePointAt(0)
  return codePoint === undefined
    || codePoint <= 0x1f
    || (codePoint >= 0x7f && codePoint <= 0x9f)
    || codePoint === 0x200e
    || codePoint === 0x200f
    || (codePoint >= 0x202a && codePoint <= 0x202e)
    || (codePoint >= 0x2066 && codePoint <= 0x2069)
}

function hasLoneSurrogate(value: string) {
  for (let index = 0; index < value.length; index += 1) {
    const current = value.charCodeAt(index)
    if (current >= 0xd800 && current <= 0xdbff) {
      const next = value.charCodeAt(index + 1)
      if (next < 0xdc00 || next > 0xdfff) return true
      index += 1
    } else if (current >= 0xdc00 && current <= 0xdfff) {
      return true
    }
  }
  return false
}

function byteLength(value: string) {
  return TEXT_ENCODER.encode(value).byteLength
}

function compareUtf8(left: string, right: string) {
  const leftBytes = TEXT_ENCODER.encode(left)
  const rightBytes = TEXT_ENCODER.encode(right)
  const length = Math.min(leftBytes.length, rightBytes.length)
  for (let index = 0; index < length; index += 1) {
    const difference = (leftBytes[index] ?? 0) - (rightBytes[index] ?? 0)
    if (difference !== 0) return difference
  }
  return leftBytes.length - rightBytes.length
}

function freezeDeep<Value>(value: Value): Value {
  if (typeof value !== 'object' || value === null || Object.isFrozen(value)) {
    return value
  }
  for (const child of Object.values(value)) freezeDeep(child)
  return Object.freeze(value)
}

function fail(code: FoldTechniqueValidationErrorV1): never {
  throw new AdmissionFailure(code)
}
