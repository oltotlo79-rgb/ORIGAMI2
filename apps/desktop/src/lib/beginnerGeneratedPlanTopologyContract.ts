import type {
  BeginnerDesignProfileV1,
  BeginnerGeneratedPlanV1,
} from './coreClient.ts'
import type {
  BeginnerGeneratedPlanKindV1,
} from './beginnerGeneratedPlanContract.ts'

const BEGINNER_ASYMMETRIC_PLAN_KINDS_V1 =
  new Set<BeginnerGeneratedPlanKindV1>([
    'asymmetric_four_leg_landmark_base',
    'asymmetric_bird_landmark_base',
    'asymmetric_insect_landmark_base',
    'asymmetric_fish_landmark_base',
  ])

function expectedContourLengthsV1(
  expectedProfile: BeginnerDesignProfileV1,
): ReadonlyArray<number> {
  const constraints = expectedProfile.generation_constraints
  return Object.freeze([
    ...(constraints.generic_body_outline_tenths_mm === undefined
      ? []
      : [constraints.generic_body_outline_tenths_mm.length]),
    ...(constraints.protrusions ?? []).flatMap((protrusion) =>
      protrusion.local_outline_tenths_mm === undefined
        ? []
        : [protrusion.local_outline_tenths_mm.length]),
  ])
}

export function beginnerGeneratedPlanTopologyMatchesProfileV1(
  plan: BeginnerGeneratedPlanV1,
  expectedProfile: BeginnerDesignProfileV1,
  planIndex: number,
  contourLengths:
    ReadonlyArray<number> = expectedContourLengthsV1(expectedProfile),
): boolean {
  const vertices = plan.crease_pattern.vertices
  const edges = plan.crease_pattern.edges
  const expectedBaseKind = expectedProfile.generation_constraints
    .allowed_techniques.includes('valley_fold')
    ? 'valley'
    : 'mountain'
  if (planIndex > 0) {
    return vertices.length === 2
      && edges.length === 1
      && edges[0]?.start === vertices[0]?.id
      && edges[0]?.end === vertices[1]?.id
      && edges[0]?.kind === expectedBaseKind
  }
  const physicalEdges = edges.filter((edge) =>
    edge.kind === 'mountain' || edge.kind === 'valley')
  const auxiliaryEdges = edges.slice(physicalEdges.length)
  const supportInstructions = plan.instruction_codes.filter((code) =>
    code.startsWith('bounded_radial_corner_support_v1:'))
  const supportMatch = supportInstructions.length === 1
    ? /^bounded_radial_corner_support_v1:added=([0-5]):covered=4$/u
        .exec(supportInstructions[0]!)
    : null
  const supportEdgeCount = supportMatch
    ? Number(supportMatch[1])
    : 0
  const basePhysicalEdgeCount =
    physicalEdges.length - supportEdgeCount
  if (
    physicalEdges.length < 1
    || supportInstructions.length > 1
    || (
      supportInstructions.length === 1
      && supportMatch === null
    )
    || basePhysicalEdgeCount < 1
    || edges.slice(0, physicalEdges.length).some((edge) =>
      edge.kind === 'auxiliary')
    || auxiliaryEdges.some((edge) => edge.kind !== 'auxiliary')
    || vertices.length < physicalEdges.length + 1
  ) return false
  const centerId = vertices[0]?.id
  const baseEndpointIds = vertices
    .slice(1, basePhysicalEdgeCount + 1)
    .map((vertex) => vertex.id)
  const supportEndpointIds = vertices
    .slice(
      basePhysicalEdgeCount + 1,
      physicalEdges.length + 1,
    )
    .map((vertex) => vertex.id)
  const asymmetric =
    BEGINNER_ASYMMETRIC_PLAN_KINDS_V1.has(plan.kind)
  if (
    !centerId
    || new Set([
      ...baseEndpointIds,
      ...supportEndpointIds,
    ]).size !== physicalEdges.length
    || supportEndpointIds.length !== supportEdgeCount
    || physicalEdges
      .slice(0, supportEdgeCount)
      .some((edge, index) =>
        edge.kind !== expectedBaseKind
        || edge.start !== centerId
        || edge.end !== supportEndpointIds[index])
    || physicalEdges
      .slice(supportEdgeCount)
      .some((edge, index) => {
        const expectedKind =
          asymmetric && index === 3 ? 'mountain' : expectedBaseKind
        return edge.kind !== expectedKind
          || (asymmetric ? edge.end : edge.start) !== centerId
          || (asymmetric ? edge.start : edge.end)
            !== baseEndpointIds[index]
      })
  ) return false

  const contourEdgeCount = contourLengths.reduce(
    (sum, length) => sum + length,
    0,
  )
  const generic = plan.kind === 'composite_generic_target_base'
  const treeEdgeCount = generic ? plan.skeleton_segments.length : 0
  if (
    auxiliaryEdges.length !== contourEdgeCount + treeEdgeCount
    || vertices.length !== edges.length + (generic ? 2 : 1)
  ) return false

  let contourVertexStart = physicalEdges.length + 1
  let contourEdgeStart = physicalEdges.length
  for (const contourLength of contourLengths) {
    const contourVertexIds = vertices
      .slice(
        contourVertexStart,
        contourVertexStart + contourLength,
      )
      .map((vertex) => vertex.id)
    const contourEdges = edges.slice(
      contourEdgeStart,
      contourEdgeStart + contourLength,
    )
    if (
      contourVertexIds.length !== contourLength
      || contourEdges.length !== contourLength
      || contourEdges.some((edge, index) =>
        edge.kind !== 'auxiliary'
        || edge.start !== contourVertexIds[index]
        || edge.end !==
          contourVertexIds[(index + 1) % contourLength])
    ) return false
    contourVertexStart += contourLength
    contourEdgeStart += contourLength
  }
  if (!generic) {
    return contourVertexStart === vertices.length
      && contourEdgeStart === edges.length
  }
  const treeVertexIds = vertices
    .slice(contourVertexStart)
    .map((vertex) => vertex.id)
  const treeEdges = edges.slice(contourEdgeStart)
  const skeletonPoints = Array.from(new Map(
    plan.skeleton_segments.flatMap((segment) => [
      segment.start,
      segment.end,
    ]).map((point) => [
      `${point.x_tenths_mm}:${point.y_tenths_mm}`,
      point,
    ] as const),
  ).values()).sort((left, right) =>
    left.x_tenths_mm - right.x_tenths_mm
    || left.y_tenths_mm - right.y_tenths_mm)
  const treeVertexIdByPoint = new Map(
    skeletonPoints.map((point, index) => [
      `${point.x_tenths_mm}:${point.y_tenths_mm}`,
      treeVertexIds[index]!,
    ] as const),
  )
  if (
    treeVertexIds.length !== treeEdgeCount + 1
    || skeletonPoints.length !== treeVertexIds.length
    || treeEdges.length !== treeEdgeCount
    || treeEdges.some((edge, index) => {
      const segment = plan.skeleton_segments[index]
      const expectedStart = segment && treeVertexIdByPoint.get(
        `${segment.start.x_tenths_mm}:${segment.start.y_tenths_mm}`,
      )
      const expectedEnd = segment && treeVertexIdByPoint.get(
        `${segment.end.x_tenths_mm}:${segment.end.y_tenths_mm}`,
      )
      return (
        edge.kind !== 'auxiliary'
        || edge.start !== expectedStart
        || edge.end !== expectedEnd
      )
    })
  ) return false
  const reached = new Set<string>()
  const pending = treeVertexIds.length === 0
    ? []
    : [treeVertexIds[0]!]
  while (pending.length > 0) {
    const current = pending.pop()!
    if (reached.has(current)) continue
    reached.add(current)
    for (const edge of treeEdges) {
      if (edge.start === current && !reached.has(edge.end)) {
        pending.push(edge.end)
      } else if (edge.end === current && !reached.has(edge.start)) {
        pending.push(edge.start)
      }
    }
  }
  return reached.size === treeVertexIds.length
}
