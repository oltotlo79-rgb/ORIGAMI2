const INVALID_STACKED_FOLD_WIRE_VALUE = Symbol(
  'invalid stacked-fold wire value',
)
const MAX_STACKED_FOLD_WIRE_CONTAINERS = 1_000_000
const MAX_STACKED_FOLD_WIRE_ENTRIES = 2_000_000
const MAX_STACKED_FOLD_WIRE_ARRAY_LENGTH = 1_000_000
const MAX_STACKED_FOLD_WIRE_RECORD_FIELDS = 64
const MAX_STACKED_FOLD_WIRE_DEPTH = 16

type StackedFoldWireSnapshotState = {
  containerCount: number
  entryCount: number
  readonly seen: WeakSet<object>
}

export type StackedFoldReadWireSnapshot = Readonly<{
  value: unknown
}>

/**
 * Detaches the complete IPC payload through own data descriptors before any
 * semantic field is read. JSON cannot contain accessors, symbols, sparse
 * arrays, shared object identity, or custom prototypes, so each is rejected
 * without invoking a user-controlled getter.
 */
export function snapshotStackedFoldReadWireValue(
  value: unknown,
): StackedFoldReadWireSnapshot | null {
  const snapshot = snapshotValueAtDepth(
    value,
    {
      containerCount: 0,
      entryCount: 0,
      seen: new WeakSet<object>(),
    },
    0,
  )
  return snapshot === INVALID_STACKED_FOLD_WIRE_VALUE
    ? null
    : Object.freeze({ value: snapshot })
}

function snapshotValueAtDepth(
  value: unknown,
  state: StackedFoldWireSnapshotState,
  depth: number,
): unknown | typeof INVALID_STACKED_FOLD_WIRE_VALUE {
  if (
    value === null
    || typeof value === 'boolean'
    || typeof value === 'number'
    || typeof value === 'string'
  ) return value
  if (typeof value !== 'object' || depth > MAX_STACKED_FOLD_WIRE_DEPTH) {
    return INVALID_STACKED_FOLD_WIRE_VALUE
  }
  if (
    state.seen.has(value)
    || state.containerCount >= MAX_STACKED_FOLD_WIRE_CONTAINERS
  ) return INVALID_STACKED_FOLD_WIRE_VALUE
  state.seen.add(value)
  state.containerCount += 1

  try {
    if (Array.isArray(value)) {
      return snapshotArray(value, state, depth)
    }
    return snapshotRecord(value, state, depth)
  } catch {
    return INVALID_STACKED_FOLD_WIRE_VALUE
  }
}

function snapshotArray(
  value: unknown[],
  state: StackedFoldWireSnapshotState,
  depth: number,
): readonly unknown[] | typeof INVALID_STACKED_FOLD_WIRE_VALUE {
  if (Object.getPrototypeOf(value) !== Array.prototype) {
    return INVALID_STACKED_FOLD_WIRE_VALUE
  }
  const descriptors = Object.getOwnPropertyDescriptors(value)
  const lengthDescriptor =
    (descriptors as unknown as PropertyDescriptorMap)['length']
  const rawLength = lengthDescriptor && 'value' in lengthDescriptor
    ? lengthDescriptor.value
    : null
  if (
    !lengthDescriptor
    || typeof rawLength !== 'number'
    || !Number.isSafeInteger(rawLength)
    || rawLength < 0
    || rawLength > MAX_STACKED_FOLD_WIRE_ARRAY_LENGTH
    || rawLength > MAX_STACKED_FOLD_WIRE_ENTRIES - state.entryCount
  ) return INVALID_STACKED_FOLD_WIRE_VALUE
  state.entryCount += rawLength
  const keys = Reflect.ownKeys(descriptors)
  if (
    keys.length !== rawLength + 1
    || keys.some((key) =>
      typeof key !== 'string'
      || (
        key !== 'length'
        && (
          !/^(?:0|[1-9][0-9]*)$/u.test(key)
          || Number(key) >= rawLength
        )
      ))
  ) return INVALID_STACKED_FOLD_WIRE_VALUE

  const snapshot: unknown[] = []
  for (let index = 0; index < rawLength; index += 1) {
    const descriptor = descriptors[String(index)]
    if (
      !descriptor
      || !descriptor.enumerable
      || !('value' in descriptor)
    ) return INVALID_STACKED_FOLD_WIRE_VALUE
    const item = snapshotValueAtDepth(
      descriptor.value,
      state,
      depth + 1,
    )
    if (item === INVALID_STACKED_FOLD_WIRE_VALUE) {
      return INVALID_STACKED_FOLD_WIRE_VALUE
    }
    snapshot.push(item)
  }
  return Object.freeze(snapshot)
}

function snapshotRecord(
  value: object,
  state: StackedFoldWireSnapshotState,
  depth: number,
): Readonly<Record<string, unknown>>
  | typeof INVALID_STACKED_FOLD_WIRE_VALUE {
  if (Object.getPrototypeOf(value) !== Object.prototype) {
    return INVALID_STACKED_FOLD_WIRE_VALUE
  }
  const descriptors = Object.getOwnPropertyDescriptors(value)
  const keys = Reflect.ownKeys(descriptors)
  if (
    keys.length > MAX_STACKED_FOLD_WIRE_RECORD_FIELDS
    || keys.length > MAX_STACKED_FOLD_WIRE_ENTRIES - state.entryCount
    || keys.some((key) => typeof key !== 'string')
  ) return INVALID_STACKED_FOLD_WIRE_VALUE
  state.entryCount += keys.length

  const snapshot: Record<string, unknown> = {}
  for (const key of keys as string[]) {
    const descriptor = descriptors[key]
    if (
      !descriptor
      || !descriptor.enumerable
      || !('value' in descriptor)
    ) return INVALID_STACKED_FOLD_WIRE_VALUE
    const field = snapshotValueAtDepth(
      descriptor.value,
      state,
      depth + 1,
    )
    if (field === INVALID_STACKED_FOLD_WIRE_VALUE) {
      return INVALID_STACKED_FOLD_WIRE_VALUE
    }
    Object.defineProperty(snapshot, key, {
      value: field,
      enumerable: true,
      configurable: false,
      writable: false,
    })
  }
  return Object.freeze(snapshot)
}
