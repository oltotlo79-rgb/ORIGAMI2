import { isCanonicalNonNilUuid } from './canonicalUuid.ts'
import type { VertexCoordinateExpressionBinding } from './coreClient.ts'

/**
 * Mirrors `sources_use_valid_edge_geometry_references` in `ori-formats`.
 *
 * This is a lexical compatibility check only. It does not resolve an edge,
 * reevaluate a coordinate, or provide project-mutation authority.
 */
export function sourcesUseValidEdgeGeometryReferences(
  xSource: string,
  ySource: string,
): boolean {
  const xCount = sourceEdgeGeometryReferenceCount(xSource)
  const yCount = sourceEdgeGeometryReferenceCount(ySource)
  if (xCount === null || yCount === null) return false
  const total = xCount + yCount
  return Number.isSafeInteger(total) && total > 0
}

/**
 * Identifies a legacy binding whose persisted coordinates remain the display
 * source of truth because its edge-derived values have not been reverified.
 */
export function isUnverifiedLegacyV1EdgeGeometryBinding(
  binding: VertexCoordinateExpressionBinding | undefined,
): boolean {
  return binding?.schema_version === 1
    && binding.transcendental_model_id === undefined
    && sourcesUseValidEdgeGeometryReferences(
      binding.x_source,
      binding.y_source,
    )
}

function sourceEdgeGeometryReferenceCount(source: string): number | null {
  let count = 0
  let cursor = 0
  while (cursor < source.length) {
    const start = source.indexOf('e.', cursor)
    if (start < 0) break

    const uuidStart = start + 2
    const uuidEnd = uuidStart + 36
    const uuid = source.slice(uuidStart, uuidEnd)
    if (uuid.length !== 36 || !isCanonicalNonNilUuid(uuid)) return null

    let tokenEnd: number
    if (source.startsWith('.length', uuidEnd)) {
      tokenEnd = uuidEnd + '.length'.length
    } else if (source.startsWith('.angle', uuidEnd)) {
      tokenEnd = uuidEnd + '.angle'.length
    } else {
      return null
    }

    const trailing = source[tokenEnd]
    if (
      trailing !== undefined
      && isAsciiReferenceContinuation(trailing)
    ) return null
    if (count === Number.MAX_SAFE_INTEGER) return null
    count += 1
    cursor = tokenEnd
  }
  return count
}

function isAsciiReferenceContinuation(character: string): boolean {
  const code = character.charCodeAt(0)
  return (
    (code >= 0x30 && code <= 0x39)
    || (code >= 0x41 && code <= 0x5a)
    || (code >= 0x61 && code <= 0x7a)
    || character === '_'
    || character === '.'
  )
}
