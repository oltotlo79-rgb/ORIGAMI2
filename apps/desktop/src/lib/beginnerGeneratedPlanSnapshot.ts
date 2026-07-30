export type BeginnerSkeletonSegmentV1 = Readonly<{
  id: number
  start: Readonly<{ x_tenths_mm: number; y_tenths_mm: number }>
  end: Readonly<{ x_tenths_mm: number; y_tenths_mm: number }>
  thickness_tenths_mm?: number
}>

export const MAX_BEGINNER_SKELETON_SEGMENTS_V1 = 16
const MAX_BEGINNER_SKELETON_COORDINATE_TENTHS_MM_V1 = 100_000

/**
 * Copies only dense, own, enumerable data elements from a plain array.
 * Accessors, sparse indices, symbols, subclass prototypes, and Proxy failures
 * are rejected before contract validation consumes any value.
 */
export function snapshotDensePlainArray(
  value: unknown,
  maximumLength: number,
): readonly unknown[] | null {
  try {
    if (
      !Array.isArray(value)
      || Object.getPrototypeOf(value) !== Array.prototype
    ) return null
    const lengthDescriptor = Reflect.getOwnPropertyDescriptor(value, 'length')
    const length = lengthDescriptor && 'value' in lengthDescriptor
      ? lengthDescriptor.value
      : null
    if (
      typeof length !== 'number'
      || !Number.isSafeInteger(length)
      || length < 0
      || length > maximumLength
    ) return null
    const keys = Reflect.ownKeys(value)
    if (
      keys.length !== length + 1
      || keys.some((key) =>
        typeof key !== 'string'
        || (
          key !== 'length'
          && (
            !/^(?:0|[1-9][0-9]*)$/u.test(key)
            || Number(key) >= length
          )
        ))
    ) return null
    const snapshot: unknown[] = []
    for (let index = 0; index < length; index += 1) {
      const descriptor = Reflect.getOwnPropertyDescriptor(
        value,
        String(index),
      )
      if (
        !descriptor
        || !('value' in descriptor)
        || !descriptor.enumerable
      ) return null
      snapshot.push(descriptor.value)
    }
    return snapshot
  } catch {
    return null
  }
}

/**
 * Copies only own enumerable data properties from a plain/null-prototype
 * record. Unknown keys, inherited state, accessors, symbols, and Proxy
 * failures are rejected.
 */
export function snapshotPlainDataRecord(
  value: unknown,
  requiredKeys: readonly string[],
  optionalKeys: readonly string[] = [],
): Readonly<Record<string, unknown>> | null {
  try {
    if (
      value === null
      || typeof value !== 'object'
      || Array.isArray(value)
    ) return null
    const prototype = Object.getPrototypeOf(value)
    if (prototype !== Object.prototype && prototype !== null) return null
    const descriptors = Object.getOwnPropertyDescriptors(value)
    const keys = Reflect.ownKeys(descriptors)
    const allowed = new Set([...requiredKeys, ...optionalKeys])
    if (
      keys.some((key) => typeof key !== 'string' || !allowed.has(key))
      || requiredKeys.some((key) => !Object.hasOwn(descriptors, key))
    ) return null
    const snapshot = Object.create(null) as Record<string, unknown>
    for (const key of keys as string[]) {
      const descriptor = descriptors[key]
      if (
        !descriptor
        || !('value' in descriptor)
        || !descriptor.enumerable
      ) return null
      snapshot[key] = descriptor.value
    }
    return snapshot
  } catch {
    return null
  }
}

export function snapshotCanonicalSkeletonSegmentsV1(
  value: unknown,
): readonly BeginnerSkeletonSegmentV1[] | null {
  const rawSegments = snapshotDensePlainArray(
    value,
    MAX_BEGINNER_SKELETON_SEGMENTS_V1,
  )
  if (!rawSegments || rawSegments.length === 0) return null
  const segments: BeginnerSkeletonSegmentV1[] = []
  for (const rawSegment of rawSegments) {
    const segment = snapshotPlainDataRecord(
      rawSegment,
      ['id', 'start', 'end'],
      ['thickness_tenths_mm'],
    )
    const start = snapshotPlainDataRecord(
      segment?.start,
      ['x_tenths_mm', 'y_tenths_mm'],
    )
    const end = snapshotPlainDataRecord(
      segment?.end,
      ['x_tenths_mm', 'y_tenths_mm'],
    )
    if (!segment || !start || !end) return null
    const coordinates = [
      start.x_tenths_mm,
      start.y_tenths_mm,
      end.x_tenths_mm,
      end.y_tenths_mm,
    ]
    if (
      !Number.isInteger(segment.id)
      || Number(segment.id) < 0
      || Number(segment.id) > 65_535
      || coordinates.some((coordinate) =>
        !Number.isInteger(coordinate)
        || Math.abs(Number(coordinate))
          > MAX_BEGINNER_SKELETON_COORDINATE_TENTHS_MM_V1)
      || (
        segment.thickness_tenths_mm !== undefined
        && (
          !Number.isInteger(segment.thickness_tenths_mm)
          || Number(segment.thickness_tenths_mm) < 1
          || Number(segment.thickness_tenths_mm) > 10_000
        )
      )
    ) return null
    segments.push({
      id: Number(segment.id),
      start: {
        x_tenths_mm: Number(start.x_tenths_mm),
        y_tenths_mm: Number(start.y_tenths_mm),
      },
      end: {
        x_tenths_mm: Number(end.x_tenths_mm),
        y_tenths_mm: Number(end.y_tenths_mm),
      },
      ...(segment.thickness_tenths_mm === undefined ? {} : {
        thickness_tenths_mm: Number(segment.thickness_tenths_mm),
      }),
    })
  }
  return segments
}
