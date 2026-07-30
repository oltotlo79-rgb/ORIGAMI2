export type CompleteInsectBindingTarget = Readonly<{
  id: number
  count: number
  symmetry: string
  direction_milli: readonly [number, number, number]
  position_tenths_mm: readonly [number, number, number]
  priority: number
}>

export type CompleteInsectBindings<T extends CompleteInsectBindingTarget> =
  Readonly<{
    wing: T
    antenna: T
    legs: readonly [T, T, T]
    ordered: readonly [T, T, T, T, T]
  }>

function uniqueMatch<T>(
  values: readonly T[],
  predicate: (value: T) => boolean,
): T | null {
  let match: T | null = null
  for (const value of values) {
    if (!predicate(value)) continue
    if (match !== null) return null
    match = value
  }
  return match
}

/**
 * Mirrors the native v1 complete-insect role predicates while keeping storage
 * order and arbitrary unique target IDs out of the presentation contract.
 */
export function resolveCompleteInsectBindings<
  T extends CompleteInsectBindingTarget,
>(
  protrusions: readonly T[],
): CompleteInsectBindings<T> | null {
  if (
    protrusions.length !== 5
    || new Set(protrusions.map(({ id }) => id)).size !== 5
  ) {
    return null
  }
  const isBilateralPair = (target: T) => (
    target.count === 2 && target.symmetry === 'bilateral'
  )
  const wing = uniqueMatch(protrusions, (target) => (
    isBilateralPair(target)
    && target.direction_milli[0] !== 0
    && target.direction_milli[1] === 0
    && target.priority === 60
  ))
  const antenna = uniqueMatch(protrusions, (target) => (
    isBilateralPair(target)
    && target.direction_milli[0] === 0
    && target.direction_milli[1] !== 0
    && target.priority === 60
  ))
  const legs = protrusions.filter((target) => (
    isBilateralPair(target)
    && target.direction_milli[0] !== 0
    && target.direction_milli[1] === 0
    && target.priority === 50
  )).sort((left, right) => (
    left.position_tenths_mm[1] - right.position_tenths_mm[1]
    || left.id - right.id
  ))
  if (
    !wing
    || !antenna
    || legs.length !== 3
    || legs.some((target, index) => (
      index > 0
      && legs[index - 1]!.position_tenths_mm[1]
        >= target.position_tenths_mm[1]
    ))
  ) {
    return null
  }

  const ordered = [wing, antenna, legs[0]!, legs[1]!, legs[2]!] as const
  if (new Set(ordered.map(({ id }) => id)).size !== 5) return null
  return Object.freeze({
    wing,
    antenna,
    legs: Object.freeze([legs[0]!, legs[1]!, legs[2]!] as const),
    ordered: Object.freeze(ordered),
  })
}
