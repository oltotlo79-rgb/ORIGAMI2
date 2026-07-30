export type CompleteAnimalBindingTarget = Readonly<{
  id: number
  count: number
  symmetry: string
  direction_milli: readonly number[]
}>

export type CompleteAnimalBindings<T extends CompleteAnimalBindingTarget> = Readonly<{
  horn: T
  tail: T
  ears: T
  legs: T
  wing: T | null
  ordered: readonly T[]
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
 * Resolves the four complete-animal roles without assigning meaning from
 * storage order. The optional wing remains the strict final target because
 * that is the native v1 complete-winged-animal contract.
 */
export function resolveCompleteAnimalBindings<
  T extends CompleteAnimalBindingTarget,
>(
  protrusions: readonly T[],
  hasWing: boolean,
): CompleteAnimalBindings<T> | null {
  const requiredLength = hasWing ? 5 : 4
  if (
    protrusions.length !== requiredLength
    || new Set(protrusions.map(({ id }) => id)).size !== requiredLength
  ) {
    return null
  }

  const wing = hasWing ? protrusions[4] ?? null : null
  if (hasWing && (wing?.count !== 2 || wing.symmetry !== 'bilateral')) {
    return null
  }
  const animalTargets = hasWing ? protrusions.slice(0, 4) : protrusions
  const horn = uniqueMatch(animalTargets, (target) => (
    target.count === 1
    && target.symmetry === 'none'
    && target.direction_milli[0] === 0
    && target.direction_milli[1] !== 0
  ))
  const tail = uniqueMatch(animalTargets, (target) => (
    target.count === 1
    && target.symmetry === 'none'
    && target.direction_milli[0] !== 0
    && target.direction_milli[1] === 0
  ))
  const ears = uniqueMatch(animalTargets, (target) => (
    target.count === 2 && target.symmetry === 'bilateral'
  ))
  const legs = uniqueMatch(animalTargets, (target) => (
    target.count === 4 && target.symmetry === 'bilateral'
  ))
  if (!horn || !tail || !ears || !legs) return null

  const ordered = wing
    ? [horn, tail, ears, legs, wing]
    : [horn, tail, ears, legs]
  if (new Set(ordered.map(({ id }) => id)).size !== requiredLength) return null
  return Object.freeze({
    horn,
    tail,
    ears,
    legs,
    wing,
    ordered: Object.freeze(ordered),
  })
}
