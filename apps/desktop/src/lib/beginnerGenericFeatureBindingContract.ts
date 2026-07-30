import { analyzeGenericSkeletonTree } from './genericSkeletonTree.ts'
import {
  MAX_BEGINNER_SKELETON_SEGMENTS_V1,
  snapshotCanonicalSkeletonSegmentsV1,
  snapshotDensePlainArray,
  snapshotPlainDataRecord,
  type BeginnerSkeletonSegmentV1,
} from './beginnerGeneratedPlanSnapshot.ts'

type BeginnerGenericFeatureBindingIdentityV1 = Readonly<{
  protrusion_id: number
  generated_feature_id: number
  endpoint_count: number
  skeleton_segment_id: number
}>

type BeginnerSkeletonBranchBindingIdentityV1 = Readonly<{
  segment_id: number
  parent_segment_id: number | null
  parent_endpoint: 'start' | 'end' | null
  child_endpoint: 'start' | 'end' | null
  generated_feature_ids: ReadonlyArray<number>
}>

export const MAX_BEGINNER_GENERIC_FEATURE_BINDINGS_V1 = 14

/**
 * Verifies the cross-runtime identity contract independently from source IDs.
 * Generated IDs are dense ordinals while source protrusion/segment IDs retain
 * their full unsigned-16-bit identity, including zero.
 */
export function beginnerGenericFeatureBindingIdentityIsCanonicalV1(
  featureBindings: ReadonlyArray<BeginnerGenericFeatureBindingIdentityV1>,
  branchBindings: ReadonlyArray<BeginnerSkeletonBranchBindingIdentityV1>,
  skeletonSegments: readonly BeginnerSkeletonSegmentV1[],
): boolean {
  const featureInputs = snapshotDensePlainArray(
    featureBindings,
    MAX_BEGINNER_GENERIC_FEATURE_BINDINGS_V1,
  )
  const branchInputs = snapshotDensePlainArray(
    branchBindings,
    MAX_BEGINNER_SKELETON_SEGMENTS_V1,
  )
  const segments = snapshotCanonicalSkeletonSegmentsV1(skeletonSegments)
  if (
    !featureInputs
    || featureInputs.length < 1
    || !branchInputs
    || !segments
    || branchInputs.length !== segments.length
    || segments.some((segment, index) =>
      segment.start.x_tenths_mm > segment.end.x_tenths_mm
      || (
        segment.start.x_tenths_mm === segment.end.x_tenths_mm
        && segment.start.y_tenths_mm > segment.end.y_tenths_mm
      )
      || (index > 0 && segments[index - 1]!.id >= segment.id))
    || analyzeGenericSkeletonTree(segments).status !== 'tree'
  ) return false
  const features = featureInputs.map((value) => snapshotPlainDataRecord(
    value,
    [
      'protrusion_id',
      'generated_feature_id',
      'endpoint_count',
      'skeleton_segment_id',
    ],
  ))
  const branches = branchInputs.map((value) => {
    const branch = snapshotPlainDataRecord(
      value,
      [
        'segment_id',
        'parent_segment_id',
        'parent_endpoint',
        'child_endpoint',
        'generated_feature_ids',
      ],
    )
    const generatedFeatureIds = snapshotDensePlainArray(
      branch?.generated_feature_ids,
      MAX_BEGINNER_GENERIC_FEATURE_BINDINGS_V1,
    )
    return branch && generatedFeatureIds
      ? {
          segment_id: branch.segment_id,
          parent_segment_id: branch.parent_segment_id,
          parent_endpoint: branch.parent_endpoint,
          child_endpoint: branch.child_endpoint,
          generated_feature_ids: generatedFeatureIds,
        }
      : null
  })
  if (
    features.some((binding) => binding === null)
    || branches.some((branch) => branch === null)
  ) return false
  const skeletonIds = segments.map((segment) => segment.id)
  const skeletonIdSet = new Set(skeletonIds)
  const protrusionIds = new Set<number>()
  let endpointTotal = 0
  for (const [index, binding] of features.entries()) {
    if (!binding
      || !Number.isInteger(binding.protrusion_id)
      || Number(binding.protrusion_id) < 0
      || Number(binding.protrusion_id) > 65_535
      || protrusionIds.has(Number(binding.protrusion_id))
      || binding.generated_feature_id !== index + 1
      || !Number.isInteger(binding.endpoint_count)
      || Number(binding.endpoint_count) < 1
      || Number(binding.endpoint_count) > 8
      || !Number.isInteger(binding.skeleton_segment_id)
      || !skeletonIdSet.has(Number(binding.skeleton_segment_id))
      || (index > 0
        && Number(features[index - 1]!.protrusion_id)
          >= Number(binding.protrusion_id))) {
      return false
    }
    endpointTotal += Number(binding.endpoint_count)
    if (endpointTotal > 32) return false
    protrusionIds.add(Number(binding.protrusion_id))
  }

  type Endpoint = 'start' | 'end'
  type ExpectedBranch = Readonly<{
    segment_id: number
    parent_segment_id: number | null
    parent_endpoint: Endpoint | null
    child_endpoint: Endpoint | null
    generated_feature_ids: readonly number[]
  }>
  const point = (segment: BeginnerSkeletonSegmentV1, endpoint: Endpoint) => (
    segment[endpoint]
  )
  const adjacentEndpoints = (
    parent: BeginnerSkeletonSegmentV1,
    child: BeginnerSkeletonSegmentV1,
  ): readonly [Endpoint, Endpoint] | null => {
    for (const parentEndpoint of ['start', 'end'] as const) {
      for (const childEndpoint of ['start', 'end'] as const) {
        const left = point(parent, parentEndpoint)
        const right = point(child, childEndpoint)
        if (
          left.x_tenths_mm === right.x_tenths_mm
          && left.y_tenths_mm === right.y_tenths_mm
        ) return [parentEndpoint, childEndpoint]
      }
    }
    return null
  }
  const featuresForSegment = (segmentId: number) => features
    .map((binding, index) => (
      Number(binding?.skeleton_segment_id) === segmentId ? index + 1 : null
    ))
    .filter((id): id is number => id !== null)
  const expectedBranches: ExpectedBranch[] = [{
    segment_id: segments[0]!.id,
    parent_segment_id: null,
    parent_endpoint: null,
    child_endpoint: null,
    generated_feature_ids: featuresForSegment(segments[0]!.id),
  }]
  const visited = new Set<number>([segments[0]!.id])
  while (visited.size < segments.length) {
    let next: ExpectedBranch | null = null
    for (const child of segments) {
      if (visited.has(child.id)) continue
      for (const parent of segments) {
        if (!visited.has(parent.id)) continue
        const endpoints = adjacentEndpoints(parent, child)
        if (!endpoints) continue
        next = {
          segment_id: child.id,
          parent_segment_id: parent.id,
          parent_endpoint: endpoints[0],
          child_endpoint: endpoints[1],
          generated_feature_ids: featuresForSegment(child.id),
        }
        break
      }
      if (next) break
    }
    if (!next || visited.has(next.segment_id)) return false
    visited.add(next.segment_id)
    expectedBranches.push(next)
  }
  return branches.every((branch, index) => {
    const expected = expectedBranches[index]
    return branch !== null
      && expected !== undefined
      && Number.isInteger(branch.segment_id)
      && Number(branch.segment_id) >= 0
      && Number(branch.segment_id) <= 65_535
      && (
        branch.parent_segment_id === null
        || (
          Number.isInteger(branch.parent_segment_id)
          && Number(branch.parent_segment_id) >= 0
          && Number(branch.parent_segment_id) <= 65_535
        )
      )
      && branch.segment_id === expected.segment_id
      && branch.parent_segment_id === expected.parent_segment_id
      && branch.parent_endpoint === expected.parent_endpoint
      && branch.child_endpoint === expected.child_endpoint
      && branch.generated_feature_ids.length
        === expected.generated_feature_ids.length
      && branch.generated_feature_ids.every((id, featureIndex) =>
        Number.isInteger(id)
        && id === expected.generated_feature_ids[featureIndex])
  })
}
