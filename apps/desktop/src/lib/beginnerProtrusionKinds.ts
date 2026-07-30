import type { BeginnerDesignProfileV1 } from './coreClient.ts'
import { MAX_BEGINNER_GENERAL_FEATURE_ENDPOINTS_V1 } from './beginnerGeneratedPlanContract.ts'
import { resolveCompleteAnimalBindings } from './completeAnimalBindings.ts'
import { resolveCompleteInsectBindings } from './completeInsectBindings.ts'

type Constraints = BeginnerDesignProfileV1['generation_constraints']
type TargetPart = Constraints['target_parts'][number]
type PartKind = TargetPart['kind']
type Protrusion = NonNullable<Constraints['protrusions']>[number]
type TargetCategory = Constraints['target_category']

export type BeginnerProtrusionKindAssignmentV1 = PartKind | null

type SemanticFamily =
  | 'specialized'
  | 'generic'
  | 'custom_direct'
  | 'empty'
  | 'invalid'

export type BeginnerSemanticFamilyV1 = SemanticFamily

type SpecializedResolution = Readonly<{
  valid: boolean
  kinds: PartKind[] | null
}>

const BODY_PART_KINDS = new Set<PartKind>(['head', 'torso'])
const FEATURE_KIND_ORDER = [
  'leg',
  'wing',
  'tail',
  'horn',
  'antenna',
  'ear',
  'fin',
] as const satisfies readonly PartKind[]

const ANIMAL_SPECIALIZED_SIGNATURES = new Set([
  'leg:4',
  'wing:2',
  'tail:1,fin:2',
  'fin:2',
  'ear:2',
  'horn:2',
  'tail:1',
  'horn:1',
  'tail:1,ear:2',
  'horn:1,ear:2',
  'tail:1,horn:1',
  'tail:1,horn:1,ear:2',
  'leg:4,tail:1,horn:1,ear:2',
  'leg:4,wing:2,tail:1,horn:1,ear:2',
])

const INSECT_SPECIALIZED_SIGNATURES = new Set([
  'leg:6,wing:2,tail:1',
  'wing:2',
  'wing:4',
  'antenna:2',
  'leg:2',
  'leg:6',
  'antenna:1',
  'wing:2,antenna:2',
  'leg:6,wing:2,antenna:2',
])

function semanticPartsV1(
  targetParts: readonly TargetPart[],
  targetCategory: TargetCategory,
): Readonly<{
  family: SemanticFamily
  features: readonly TargetPart[]
  featureTotal: number
  signature: string
}> {
  if (targetParts.length > 10
    || new Set(targetParts.map((part) => part.kind)).size
      !== targetParts.length
    || targetParts.some((part) => (
      !Number.isInteger(part.count)
      || part.count < 1
      || part.count > 8
    ))) {
    return { family: 'invalid', features: [], featureTotal: 0, signature: '' }
  }
  const total = targetParts.reduce((sum, part) => sum + part.count, 0)
  if (!Number.isSafeInteger(total) || total > 32) {
    return { family: 'invalid', features: [], featureTotal: 0, signature: '' }
  }
  const count = (kind: PartKind) =>
    targetParts.find((part) => part.kind === kind)?.count ?? 0
  if (targetCategory !== 'custom_object'
    && targetCategory !== null
    && (count('head') !== 1 || count('torso') !== 1)) {
    return { family: 'invalid', features: [], featureTotal: 0, signature: '' }
  }
  if (targetCategory === null && targetParts.length > 0) {
    return { family: 'invalid', features: [], featureTotal: 0, signature: '' }
  }
  const features = targetParts.filter(
    (part) => !BODY_PART_KINDS.has(part.kind),
  )
  const canonicalFeatures = FEATURE_KIND_ORDER.flatMap((kind) => {
    const part = targetParts.find((candidate) => candidate.kind === kind)
    return part ? [part] : []
  })
  const featureTotal = features.reduce((sum, part) => sum + part.count, 0)
  const signature = canonicalFeatures.map(
    (part) => `${part.kind}:${part.count}`,
  ).join(',')
  if (targetCategory === 'custom_object') {
    return {
      family: featureTotal >= 2 && featureTotal <= MAX_BEGINNER_GENERAL_FEATURE_ENDPOINTS_V1
        ? 'generic'
        : 'custom_direct',
      features,
      featureTotal,
      signature,
    }
  }
  if (targetCategory === null) {
    return {
      family: targetParts.length === 0 ? 'empty' : 'invalid',
      features,
      featureTotal,
      signature,
    }
  }
  const specialized = targetCategory === 'animal'
    ? ANIMAL_SPECIALIZED_SIGNATURES.has(signature)
    : INSECT_SPECIALIZED_SIGNATURES.has(signature)
  return {
    family: specialized
      ? 'specialized'
      : featureTotal >= 2 && featureTotal <= MAX_BEGINNER_GENERAL_FEATURE_ENDPOINTS_V1
        ? 'generic'
        : featureTotal === 0
          ? 'empty'
          : 'invalid',
    features,
    featureTotal,
    signature,
  }
}

function physicalTargetsAreBoundedV1(
  protrusions: readonly Protrusion[],
): boolean {
  return protrusions.length <= 32
    && new Set(protrusions.map((target) => target.id)).size
      === protrusions.length
    && protrusions.every((target) => (
      Number.isInteger(target.count)
      && target.count >= 1
      && target.count <= 8
    ))
}

function endpointTotalV1(protrusions: readonly Protrusion[]): number | null {
  const total = protrusions.reduce((sum, target) => sum + target.count, 0)
  return Number.isSafeInteger(total) && total <= 32 ? total : null
}

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

function kindsByIdV1(
  protrusions: readonly Protrusion[],
  bindings: readonly Readonly<{
    target: Protrusion
    kind: PartKind
  }>[],
): PartKind[] | null {
  if (bindings.length !== protrusions.length) return null
  const byId = new Map(bindings.map(({ target, kind }) => [target.id, kind]))
  if (byId.size !== protrusions.length) return null
  const kinds = protrusions.map((target) => byId.get(target.id) ?? null)
  return kinds.every((kind): kind is PartKind => kind !== null)
    ? kinds
    : null
}

function specializedResolutionV1(
  semantic: ReturnType<typeof semanticPartsV1>,
  protrusions: readonly Protrusion[],
  targetCategory: TargetCategory,
): SpecializedResolution {
  if (!physicalTargetsAreBoundedV1(protrusions)) {
    return { valid: false, kinds: null }
  }
  const featureCounts = new Map(
    semantic.features.map((part) => [part.kind, part.count]),
  )
  const onlyKind = semantic.features.length === 1
    ? semantic.features[0]?.kind
    : undefined
  if (onlyKind !== undefined) {
    const oneBilateral = protrusions.length === 1
      && protrusions[0]?.count === semantic.featureTotal
      && protrusions[0]?.symmetry === 'bilateral'
    const orderedSingletons = protrusions.length === semantic.featureTotal
      && protrusions.every((target) => (
        target.count === 1 && target.symmetry === 'none'
      ))
    const threePairs = protrusions.length === 3
      && protrusions.every((target) => (
        target.count === 2 && target.symmetry === 'bilateral'
      ))
    const oneCenterAxis = protrusions.length === 1
      && protrusions[0]?.count === 1
      && protrusions[0]?.symmetry === 'none'
    const valid = endpointTotalV1(protrusions) === semantic.featureTotal
      && (
        targetCategory === 'animal'
          ? onlyKind === 'leg'
            ? oneBilateral || orderedSingletons
            : onlyKind === 'wing'
              ? oneBilateral || orderedSingletons
              : onlyKind === 'tail'
                ? oneCenterAxis
                  && protrusions[0]!.direction_milli[0] !== 0
                  && protrusions[0]!.direction_milli[1] === 0
                : onlyKind === 'horn' && semantic.featureTotal === 1
                  ? oneCenterAxis
                    && protrusions[0]!.direction_milli[0] === 0
                    && protrusions[0]!.direction_milli[1] !== 0
                  : oneBilateral
          : onlyKind === 'leg' && semantic.featureTotal === 6
            ? threePairs
            : onlyKind === 'antenna' && semantic.featureTotal === 1
              ? oneCenterAxis
                && protrusions[0]!.direction_milli[0] === 0
                && protrusions[0]!.direction_milli[1] !== 0
              : oneBilateral
      )
    return {
      valid,
      kinds: valid ? protrusions.map(() => onlyKind) : null,
    }
  }

  if (semantic.signature === 'leg:6,wing:2,tail:1') {
    const valid = protrusions.length === 7
      && protrusions.every((target) => (
        target.count === 1 && target.symmetry === 'none'
      ))
    return { valid, kinds: null }
  }
  if (semantic.signature === 'tail:1,fin:2') {
    const valid = protrusions.length === 3
      && protrusions.every((target) => (
        target.count === 1 && target.symmetry === 'none'
      ))
    return { valid, kinds: null }
  }

  if (featureCounts.has('horn')
    && featureCounts.has('tail')
    && featureCounts.has('ear')) {
    const hasLegs = featureCounts.get('leg') === 4
    const hasWing = featureCounts.get('wing') === 2
    if (hasLegs) {
      const bindings = resolveCompleteAnimalBindings(protrusions, hasWing)
      if (!bindings) return { valid: false, kinds: null }
      const resolved = [
        { target: bindings.horn, kind: 'horn' as const },
        { target: bindings.tail, kind: 'tail' as const },
        { target: bindings.ears, kind: 'ear' as const },
        { target: bindings.legs, kind: 'leg' as const },
        ...(bindings.wing
          ? [{ target: bindings.wing, kind: 'wing' as const }]
          : []),
      ]
      return { valid: true, kinds: kindsByIdV1(protrusions, resolved) }
    }
    if (protrusions.length !== 3) return { valid: false, kinds: null }
    const horn = uniqueMatch(protrusions, (target) => (
      target.count === 1
      && target.symmetry === 'none'
      && target.direction_milli[0] === 0
      && target.direction_milli[1] !== 0
    ))
    const tail = uniqueMatch(protrusions, (target) => (
      target.count === 1
      && target.symmetry === 'none'
      && target.direction_milli[0] !== 0
      && target.direction_milli[1] === 0
    ))
    const ears = uniqueMatch(protrusions, (target) => (
      target.count === 2 && target.symmetry === 'bilateral'
    ))
    if (!horn || !tail || !ears) return { valid: false, kinds: null }
    const kinds = kindsByIdV1(protrusions, [
      { target: horn, kind: 'horn' },
      { target: tail, kind: 'tail' },
      { target: ears, kind: 'ear' },
    ])
    return { valid: kinds !== null, kinds }
  }

  if (semantic.signature === 'tail:1,horn:1') {
    const horn = uniqueMatch(protrusions, (target) => (
      target.count === 1
      && target.symmetry === 'none'
      && target.direction_milli[0] === 0
      && target.direction_milli[1] !== 0
    ))
    const tail = uniqueMatch(protrusions, (target) => (
      target.count === 1
      && target.symmetry === 'none'
      && target.direction_milli[0] !== 0
      && target.direction_milli[1] === 0
    ))
    const kinds = horn && tail
      ? kindsByIdV1(protrusions, [
          { target: horn, kind: 'horn' },
          { target: tail, kind: 'tail' },
        ])
      : null
    return { valid: kinds !== null, kinds }
  }

  if (semantic.signature === 'tail:1,ear:2'
    || semantic.signature === 'horn:1,ear:2') {
    const singletonKind = semantic.signature.startsWith('tail')
      ? 'tail' as const
      : 'horn' as const
    const singleton = uniqueMatch(protrusions, (target) => (
      target.count === 1 && target.symmetry === 'none'
    ))
    const ears = uniqueMatch(protrusions, (target) => (
      target.count === 2 && target.symmetry === 'bilateral'
    ))
    const kinds = singleton && ears
      ? kindsByIdV1(protrusions, [
          { target: singleton, kind: singletonKind },
          { target: ears, kind: 'ear' },
        ])
      : null
    return { valid: kinds !== null, kinds }
  }

  if (semantic.signature === 'wing:2,antenna:2') {
    const wing = uniqueMatch(protrusions, (target) => (
      target.count === 2
      && target.symmetry === 'bilateral'
      && target.direction_milli[0] !== 0
      && target.direction_milli[1] === 0
    ))
    const antenna = uniqueMatch(protrusions, (target) => (
      target.count === 2
      && target.symmetry === 'bilateral'
      && target.direction_milli[0] === 0
      && target.direction_milli[1] !== 0
    ))
    const kinds = wing && antenna
      ? kindsByIdV1(protrusions, [
          { target: wing, kind: 'wing' },
          { target: antenna, kind: 'antenna' },
        ])
      : null
    return { valid: kinds !== null, kinds }
  }

  if (semantic.signature === 'leg:6,wing:2,antenna:2') {
    const bindings = resolveCompleteInsectBindings(protrusions)
    if (!bindings) return { valid: false, kinds: null }
    const kinds = kindsByIdV1(protrusions, [
      { target: bindings.wing, kind: 'wing' },
      { target: bindings.antenna, kind: 'antenna' },
      ...bindings.legs.map((target) => ({
        target,
        kind: 'leg' as const,
      })),
    ])
    return { valid: kinds !== null, kinds }
  }

  return { valid: false, kinds: null }
}

function orderedGenericKindsV1(
  semantic: ReturnType<typeof semanticPartsV1>,
  protrusions: readonly Protrusion[],
): PartKind[] | null {
  if (!physicalTargetsAreBoundedV1(protrusions)
    || endpointTotalV1(protrusions) !== semantic.featureTotal) return null
  const kinds: PartKind[] = []
  let featureIndex = 0
  let remaining = semantic.features[0]?.count ?? 0
  for (const target of protrusions) {
    while (remaining === 0 && featureIndex < semantic.features.length) {
      featureIndex += 1
      remaining = semantic.features[featureIndex]?.count ?? 0
    }
    const feature = semantic.features[featureIndex]
    if (!feature || target.count > remaining) return null
    kinds.push(feature.kind)
    remaining -= target.count
  }
  while (remaining === 0 && featureIndex < semantic.features.length) {
    featureIndex += 1
    remaining = semantic.features[featureIndex]?.count ?? 0
  }
  return featureIndex === semantic.features.length && remaining === 0
    ? kinds
    : null
}

/**
 * Restores display/edit bindings only when the binding rule is explicit.
 * Specialized families use native-equivalent role predicates. Ordered count
 * consumption is opt-in for first-party generic reference/recognition data;
 * arbitrary persisted/manual generic records remain unassigned.
 */
export function resolveBeginnerProtrusionKindsV1(
  targetParts: readonly TargetPart[],
  protrusions: readonly Protrusion[],
  options: Readonly<{
    targetCategory: TargetCategory
    allowOrderedGeneric?: boolean
  }>,
): PartKind[] | null {
  const semantic = semanticPartsV1(targetParts, options.targetCategory)
  if (semantic.family === 'specialized') {
    return specializedResolutionV1(
      semantic,
      protrusions,
      options.targetCategory,
    ).kinds
  }
  if (semantic.family === 'generic' && options.allowOrderedGeneric) {
    return orderedGenericKindsV1(semantic, protrusions)
  }
  if ((semantic.family === 'empty' || semantic.family === 'custom_direct')
    && protrusions.length === 0) return []
  return null
}

/**
 * Validates semantic/physical family coherence without inventing a per-record
 * kind binding. This is what permits exact no-op round trips for asymmetric
 * landmark and legacy generic profiles while rejecting persisted subsets.
 */
export function beginnerSemanticPhysicalProfileIsAdmissibleV1(
  targetParts: readonly TargetPart[],
  protrusions: readonly Protrusion[],
  targetCategory: TargetCategory,
): boolean {
  const semantic = semanticPartsV1(targetParts, targetCategory)
  if (!physicalTargetsAreBoundedV1(protrusions)) return false
  if (semantic.family === 'specialized') {
    // The persisted profile contract permits semantic-only specialized
    // targets and measured reference geometry that is not a one-record-per-
    // semantic-endpoint binding. Generation revalidates the selected family;
    // editor hydration must not invent a stricter storage contract.
    return true
  }
  if (semantic.family === 'generic') {
    return endpointTotalV1(protrusions) === semantic.featureTotal
  }
  if (semantic.family === 'custom_direct') {
    return protrusions.length === 0
      || semantic.featureTotal === 0
      || endpointTotalV1(protrusions) === semantic.featureTotal
  }
  return semantic.family === 'empty' && protrusions.length === 0
}

export function beginnerSemanticFamilyV1(
  targetParts: readonly TargetPart[],
  targetCategory: TargetCategory,
): BeginnerSemanticFamilyV1 {
  return semanticPartsV1(targetParts, targetCategory).family
}

export function beginnerTargetPartsHaveSameSemanticsV1(
  left: readonly TargetPart[],
  right: readonly TargetPart[],
): boolean {
  if (left.length !== right.length
    || new Set(left.map((part) => part.kind)).size !== left.length
    || new Set(right.map((part) => part.kind)).size !== right.length) {
    return false
  }
  const rightCounts = new Map(right.map((part) => [part.kind, part.count]))
  return left.every((part) => rightCounts.get(part.kind) === part.count)
}
