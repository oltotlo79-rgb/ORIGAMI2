const CANONICAL_NON_NIL_UUID_PATTERN =
  /^(?!00000000-0000-0000-0000-000000000000$)[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/u

/**
 * Matches the canonical lowercase text emitted by Rust's `uuid` serializer.
 *
 * UUID version and variant bits are intentionally unrestricted: persisted
 * project/entity IDs accept every non-nil UUID value at the Rust boundary.
 */
export function isCanonicalNonNilUuid(value: unknown): value is string {
  return typeof value === 'string' && CANONICAL_NON_NIL_UUID_PATTERN.test(value)
}
