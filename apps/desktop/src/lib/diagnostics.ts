export const DIAGNOSTIC_SCOPES = Object.freeze([
  'app.unhandled_error',
  'app.unhandled_rejection',
  'app.project_snapshot',
  'app.topology_analysis',
  'app.close_guard',
  'app.validation',
  'app.benchmark',
  'fold_preview.geometry',
  'fold_preview.render',
  'fold_preview.scene_initialization',
  'fold_preview.pose_application',
  'fold_preview.pose_schedule',
  'fold_preview.selection_render',
  'fold_preview.camera',
  'fold_preview.resize',
] as const)

export const REDACTED_DIAGNOSTICS_SCHEMA =
  'origami2.redacted-diagnostics.v1' as const
export const MAX_SERIALIZED_DIAGNOSTICS_BYTES = 8 * 1024

export type DiagnosticScope = (typeof DIAGNOSTIC_SCOPES)[number]

export type DiagnosticCountBucket =
  | '0'
  | '1'
  | '2_4'
  | '5_16'
  | '17_64'
  | '65_plus'

export type DiagnosticCount = Readonly<{
  scope: DiagnosticScope
  count: DiagnosticCountBucket
}>

export type DiagnosticsSnapshot = Readonly<{
  schema: typeof REDACTED_DIAGNOSTICS_SCHEMA
  unexpected: readonly DiagnosticCount[]
}>

export type Diagnostics = Readonly<{
  reportUnexpected: (scope: DiagnosticScope) => boolean
  snapshot: () => DiagnosticsSnapshot
  serialize: () => string
  clear: () => void
}>

const MAX_COUNT = 65
const UTF8_ENCODER = new TextEncoder()
const SCOPE_INDEX = new Map<DiagnosticScope, number>(
  DIAGNOSTIC_SCOPES.map((scope, index) => [scope, index]),
)

export function createDiagnostics(): Diagnostics {
  const counts = new Uint8Array(DIAGNOSTIC_SCOPES.length)

  const snapshot = (): DiagnosticsSnapshot => {
    const unexpected = DIAGNOSTIC_SCOPES.map((scope, index) =>
      Object.freeze({
        scope,
        count: countBucket(counts[index] ?? 0),
      }))
    return Object.freeze({
      schema: REDACTED_DIAGNOSTICS_SCHEMA,
      unexpected: Object.freeze(unexpected),
    })
  }

  const serialize = () => {
    const serialized = JSON.stringify(snapshot())
    if (
      UTF8_ENCODER.encode(serialized).byteLength
        > MAX_SERIALIZED_DIAGNOSTICS_BYTES
    ) {
      throw new RangeError('redacted diagnostics exceeded its fixed size limit')
    }
    return serialized
  }

  return Object.freeze({
    reportUnexpected: (scope: DiagnosticScope) => {
      if (typeof scope !== 'string') return false
      const index = SCOPE_INDEX.get(scope)
      if (index === undefined) return false
      if ((counts[index] ?? 0) < MAX_COUNT) counts[index] += 1
      return true
    },
    snapshot,
    serialize,
    clear: () => counts.fill(0),
  })
}

const applicationDiagnostics = createDiagnostics()

export function reportUnexpected(scope: DiagnosticScope) {
  return applicationDiagnostics.reportUnexpected(scope)
}

export function snapshotDiagnostics() {
  return applicationDiagnostics.snapshot()
}

export function serializeDiagnostics() {
  return applicationDiagnostics.serialize()
}

export function clearDiagnostics() {
  applicationDiagnostics.clear()
}

function countBucket(count: number): DiagnosticCountBucket {
  if (count === 0) return '0'
  if (count === 1) return '1'
  if (count <= 4) return '2_4'
  if (count <= 16) return '5_16'
  if (count <= 64) return '17_64'
  return '65_plus'
}
