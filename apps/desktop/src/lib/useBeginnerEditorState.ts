import {
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react'

import type {
  BeginnerDesignProfileV1,
  ProjectSnapshot,
} from './coreClient.ts'
import { resolveBeginnerSkeletonEndpointV1 } from './beginnerSkeletonEndpointClient.ts'
import { analyzeGenericSkeletonTree } from './genericSkeletonTree.ts'

type Constraints = BeginnerDesignProfileV1['generation_constraints']
type SkeletonSegments = Constraints['skeleton_segments']
type ComponentBridgeOverride = Constraints['component_bridge_override']
type Protrusions = NonNullable<Constraints['protrusions']>
type ProtrusionKind = Constraints['target_parts'][number]['kind']
type BulgeTargets = NonNullable<Constraints['bulge_targets']>
type BodyOutlineMode = 'symmetric' | 'general'

type BeginnerSkeletonEndpointSnapshotBinding = Readonly<{
  projectInstanceId: ProjectSnapshot['project_instance_id']
  projectId: ProjectSnapshot['project_id']
  revision: ProjectSnapshot['revision']
}>

type BeginnerSkeletonEndpointEpoch = {
  projectInstanceId: ProjectSnapshot['project_instance_id'] | undefined
  projectId: ProjectSnapshot['project_id'] | undefined
  revision: ProjectSnapshot['revision'] | undefined
  epoch: number
  commitTail: Promise<void>
}

export function useBeginnerEditorState(input: Readonly<{
  snapshot: ProjectSnapshot | null
  getCurrentSnapshot: () => ProjectSnapshot | null
  getSelectedFaceId: () => string | null
  resolveSkeletonEndpoint?: typeof resolveBeginnerSkeletonEndpointV1
}>) {
  const [beginnerPartTotal, setBeginnerPartTotal] = useState(0)
  const [beginnerSkeletonSegments, setBeginnerSkeletonSegments] =
    useState<SkeletonSegments>([])
  const [
    beginnerComponentBridgeOverride,
    setBeginnerComponentBridgeOverride,
  ] = useState<ComponentBridgeOverride>()
  const [beginnerProtrusions, setBeginnerProtrusions] =
    useState<Protrusions>([])
  const [beginnerBodyOutline, setBeginnerBodyOutline] =
    useState<Array<[number, number]>>([])
  const [beginnerBodySize, setBeginnerBodySize] =
    useState<[number, number] | undefined>()
  const [beginnerBodyOutlineMode, setBeginnerBodyOutlineMode] =
    useState<BodyOutlineMode>('symmetric')
  const [beginnerProtrusionKinds, setBeginnerProtrusionKinds] =
    useState<ProtrusionKind[]>([])
  const [beginnerBulgeTargets, setBeginnerBulgeTargets] =
    useState<BulgeTargets>([])
  const beginnerDesignFormRef = useRef<HTMLFormElement>(null)
  const snapshotRef = useRef(input.snapshot)
  snapshotRef.current = input.snapshot
  const getCurrentSnapshotRef = useRef(input.getCurrentSnapshot)
  getCurrentSnapshotRef.current = input.getCurrentSnapshot
  const snapshotProjectInstanceId = input.snapshot?.project_instance_id
  const snapshotProjectId = input.snapshot?.project_id
  const snapshotRevision = input.snapshot?.revision
  const beginnerSkeletonEndpointEpochRef = useRef<
    BeginnerSkeletonEndpointEpoch
  >({
    projectInstanceId: snapshotProjectInstanceId,
    projectId: snapshotProjectId,
    revision: snapshotRevision,
    epoch: 0,
    commitTail: Promise.resolve(),
  })
  if (!matchesEndpointEpochIdentity(
    beginnerSkeletonEndpointEpochRef.current,
    snapshotProjectInstanceId,
    snapshotProjectId,
    snapshotRevision,
  )) {
    beginnerSkeletonEndpointEpochRef.current = {
      projectInstanceId: snapshotProjectInstanceId,
      projectId: snapshotProjectId,
      revision: snapshotRevision,
      epoch: beginnerSkeletonEndpointEpochRef.current.epoch + 1,
      commitTail: Promise.resolve(),
    }
  }
  const beginnerSkeletonTree = useMemo(
    () => analyzeGenericSkeletonTree(beginnerSkeletonSegments),
    [beginnerSkeletonSegments],
  )

  useEffect(() => {
    const constraints = snapshotRef.current?.beginner_design_profile
      .generation_constraints
    setBeginnerPartTotal(
      constraints?.target_parts.reduce(
        (sum, part) => sum + part.count,
        0,
      ) ?? 0,
    )
    setBeginnerSkeletonSegments(constraints?.skeleton_segments ?? [])
    setBeginnerComponentBridgeOverride(
      constraints?.component_bridge_override,
    )
    setBeginnerProtrusions(constraints?.protrusions ?? [])
    setBeginnerBodyOutline(
      constraints?.generic_body_outline_tenths_mm
        ?.map((point) => [...point] as [number, number]) ?? [],
    )
    setBeginnerBodySize(
      constraints?.generic_body_size_tenths_mm
        ? [...constraints.generic_body_size_tenths_mm] as [number, number]
        : undefined,
    )
    setBeginnerBodyOutlineMode(
      constraints?.generic_body_outline_mode === 'general'
        ? 'general'
        : 'symmetric',
    )
    setBeginnerProtrusionKinds(
      constraints?.target_parts
        .filter((part) => part.kind !== 'head' && part.kind !== 'torso')
        .map((part) => part.kind) ?? [],
    )
    setBeginnerBulgeTargets(constraints?.bulge_targets ?? [])
  }, [snapshotProjectInstanceId, snapshotProjectId, snapshotRevision])

  function addBeginnerSkeletonSegment(form: HTMLFormElement) {
    if (beginnerSkeletonSegments.length >= 64) return
    const data = new FormData(form)
    const startX = Number(data.get('skeleton_start_x_mm'))
    const startY = Number(data.get('skeleton_start_y_mm'))
    const length = Number(data.get('skeleton_length_mm'))
    const angle = Number(data.get('skeleton_angle_degrees'))
    const thickness = Number(data.get('skeleton_thickness_mm'))
    if (
      ![startX, startY, length, angle, thickness].every(Number.isFinite)
      || Math.abs(startX) > 10_000
      || Math.abs(startY) > 10_000
      || length < 0.1
      || length > 10_000
      || angle < -360
      || angle > 360
      || thickness < 0.1
      || thickness > 1_000
    ) return
    const requestSnapshot = getCurrentSnapshotRef.current()
    const epoch = beginnerSkeletonEndpointEpochRef.current
    if (!requestSnapshot || !matchesEndpointEpochSnapshot(epoch, requestSnapshot)) {
      return
    }
    const requestBinding = snapshotBinding(requestSnapshot)
    const resolveEndpoint =
      input.resolveSkeletonEndpoint ?? resolveBeginnerSkeletonEndpointV1
    let endpointPromise: ReturnType<typeof resolveBeginnerSkeletonEndpointV1>
    try {
      endpointPromise = Promise.resolve(resolveEndpoint({
        startXMm: startX,
        startYMm: startY,
        lengthMm: length,
        angleDegrees: angle,
      }))
    } catch {
      return
    }
    const commit = epoch.commitTail.then(async () => {
      let endpoint: Awaited<
        ReturnType<typeof resolveBeginnerSkeletonEndpointV1>
      >
      try {
        endpoint = await endpointPromise
      } catch {
        return
      }
      if (beginnerSkeletonEndpointEpochRef.current !== epoch) return
      const currentSnapshot = getCurrentSnapshotRef.current()
      if (
        !currentSnapshot
        || !matchesSnapshotBinding(currentSnapshot, requestBinding)
      ) return
      setBeginnerSkeletonSegments((segments) => {
        if (segments.length >= 64) return segments
        const used = new Set(segments.map((segment) => segment.id))
        let id = 0
        while (used.has(id) && id < 65_535) id += 1
        return [...segments, {
          id,
          start: {
            x_tenths_mm: endpoint.start_tenths_mm[0],
            y_tenths_mm: endpoint.start_tenths_mm[1],
          },
          end: {
            x_tenths_mm: endpoint.end_tenths_mm[0],
            y_tenths_mm: endpoint.end_tenths_mm[1],
          },
          thickness_tenths_mm: Math.round(thickness * 10),
        }]
      })
    })
    epoch.commitTail = commit.catch(() => undefined)
  }

  function addBeginnerProtrusion(form: HTMLFormElement) {
    if (beginnerProtrusions.length >= 32) return
    const data = new FormData(form)
    const number = (name: string) => Number(data.get(name))
    const count = number('protrusion_count')
    const length = number('protrusion_length_mm')
    const thickness = number('protrusion_thickness_mm')
    const optionalWidth = (name: string) => {
      const raw = String(data.get(name) ?? '').trim()
      return raw === '' ? undefined : Number(raw)
    }
    const rootWidth = optionalWidth('protrusion_root_width_mm')
    const tipWidth = optionalWidth('protrusion_tip_width_mm')
    const position = ['x', 'y', 'z'].map(
      (axis) => Math.round(number(`protrusion_position_${axis}_mm`) * 10),
    )
    const direction = ['x', 'y', 'z'].map(
      (axis) => Math.round(number(`protrusion_direction_${axis}`) * 1_000),
    )
    const curvature = number('protrusion_curvature_degrees')
    const motion = [
      number('protrusion_motion_min'),
      number('protrusion_motion_max'),
    ]
    const priority = number('protrusion_priority')
    const symmetry = String(data.get('protrusion_symmetry'))
    const symmetryCountSupported = symmetry === 'none'
      ? count === 1
      : symmetry === 'bilateral'
        ? [2, 4, 6, 8].includes(count)
        : symmetry === 'radial' && count >= 2 && count <= 8
    if (
      ![
        count,
        length,
        thickness,
        ...position,
        ...direction,
        curvature,
        ...motion,
        priority,
      ].every(Number.isFinite)
      || !Number.isInteger(count)
      || count < 1
      || count > 8
      || !symmetryCountSupported
      || length <= 0
      || length > 100_000
      || thickness <= 0
      || thickness > 1_000
      || [rootWidth, tipWidth].some((width) => (
        width !== undefined
        && (!Number.isFinite(width) || width <= 0 || width > 1_000)
      ))
      || position.some((value) => Math.abs(value) > 100_000)
      || direction.some((value) => Math.abs(value) > 1_000)
      || direction.every((value) => value === 0)
      || Math.abs(curvature) > 360
      || motion.some((value) => Math.abs(value) > 360)
      || motion[0]! > motion[1]!
      || !Number.isInteger(priority)
      || priority < 1
      || priority > 100
    ) return
    const used = new Set(beginnerProtrusions.map((target) => target.id))
    const maximumId = beginnerProtrusions.reduce(
      (maximum, target) => Math.max(maximum, target.id),
      0,
    )
    const idsStrictlyIncreasing = beginnerProtrusions.every(
      (target, index) => index === 0
        || beginnerProtrusions[index - 1]!.id < target.id,
    )
    let id: number
    let insertionIndex = beginnerProtrusions.length
    if (maximumId < 65_535) {
      id = maximumId + 1
    } else {
      if (!idsStrictlyIncreasing) return
      id = 1
      while (used.has(id) && id < 65_535) id += 1
      if (used.has(id)) return
      insertionIndex = beginnerProtrusions.findIndex(
        (target) => target.id > id,
      )
      if (insertionIndex < 0) insertionIndex = beginnerProtrusions.length
    }
    const nextTarget: Protrusions[number] = {
      id,
      count,
      length_tenths_mm: Math.round(length * 10),
      thickness_tenths_mm: Math.round(thickness * 10),
      ...(rootWidth === undefined
        ? {}
        : { root_width_tenths_mm: Math.round(rootWidth * 10) }),
      ...(tipWidth === undefined
        ? {}
        : { tip_width_tenths_mm: Math.round(tipWidth * 10) }),
      position_tenths_mm: position as [number, number, number],
      direction_milli: direction as [number, number, number],
      symmetry: symmetry as 'none' | 'bilateral' | 'radial',
      curvature_degrees: Math.round(curvature),
      joint: String(data.get('protrusion_joint')) as
        'fixed' | 'hinge' | 'ball',
      motion_degrees: motion.map(Math.round) as [number, number],
      side: String(data.get('protrusion_side')) as
        'front' | 'back' | 'either',
      priority,
    }
    setBeginnerProtrusions((targets) => [
      ...targets.slice(0, insertionIndex),
      nextTarget,
      ...targets.slice(insertionIndex),
    ])
    setBeginnerProtrusionKinds((kinds) => {
      const nextKinds = beginnerProtrusions.map(
        (_, index) => kinds[index] ?? 'tail',
      )
      nextKinds.splice(insertionIndex, 0, 'tail')
      return nextKinds
    })
  }

  function createEmptyGenericTarget() {
    if (beginnerProtrusions.length !== 0) return
    const base: Protrusions[number] = {
      id: 1,
      count: 1,
      length_tenths_mm: 200,
      thickness_tenths_mm: 20,
      position_tenths_mm: [0, 0, 0],
      direction_milli: [0, 1_000, 0],
      symmetry: 'none',
      curvature_degrees: 0,
      joint: 'fixed',
      motion_degrees: [0, 0],
      side: 'either',
      priority: 50,
    }
    setBeginnerProtrusions([
      base,
      { ...base, id: 2, direction_milli: [1_000, 0, 0] },
    ])
    setBeginnerProtrusionKinds(['tail', 'fin'])
  }

  function addBeginnerBulgeTarget(form: HTMLFormElement) {
    const current = input.getCurrentSnapshot()
    const selectedFaceId = input.getSelectedFaceId()
    if (!current || !selectedFaceId || beginnerBulgeTargets.length >= 32) {
      return
    }
    const data = new FormData(form)
    const tuple = (prefix: string, scale: number) => (
      ['x', 'y', 'z'].map(
        (axis) => Math.round(Number(data.get(`${prefix}_${axis}`)) * scale),
      ) as [number, number, number]
    )
    const minimum = tuple('bulge_min', 10)
    const maximum = tuple('bulge_max', 10)
    const direction = tuple('bulge_direction', 1_000)
    const amount = Math.round(Number(data.get('bulge_amount_mm')) * 10)
    if (
      [...minimum, ...maximum, ...direction, amount]
        .some((value) => !Number.isFinite(value))
      || minimum.some(
        (value, index) => value > maximum[index]! || Math.abs(value) > 100_000,
      )
      || maximum.some((value) => Math.abs(value) > 100_000)
      || minimum.every((value, index) => value === maximum[index])
      || direction.some((value) => Math.abs(value) > 1_000)
      || direction.every((value) => value === 0)
      || amount < 1
      || amount > 1_000_000
    ) return
    const used = new Set(beginnerBulgeTargets.map((target) => target.id))
    let id = 0
    while (used.has(id) && id < 65_535) id += 1
    setBeginnerBulgeTargets((targets) => [...targets, {
      id,
      face_ids: [selectedFaceId],
      range_min_tenths_mm: minimum,
      range_max_tenths_mm: maximum,
      direction_milli: direction,
      amount_tenths_mm: amount,
      source_fold_model_fingerprint: current.fold_model_fingerprint,
    }])
  }

  return {
    beginnerDesignFormRef,
    beginnerPartTotal,
    setBeginnerPartTotal,
    beginnerSkeletonSegments,
    setBeginnerSkeletonSegments,
    beginnerSkeletonTree,
    beginnerComponentBridgeOverride,
    setBeginnerComponentBridgeOverride,
    beginnerProtrusions,
    setBeginnerProtrusions,
    beginnerBodyOutline,
    setBeginnerBodyOutline,
    beginnerBodySize,
    setBeginnerBodySize,
    beginnerBodyOutlineMode,
    setBeginnerBodyOutlineMode,
    beginnerProtrusionKinds,
    setBeginnerProtrusionKinds,
    beginnerBulgeTargets,
    setBeginnerBulgeTargets,
    addBeginnerSkeletonSegment,
    addBeginnerProtrusion,
    createEmptyGenericTarget,
    addBeginnerBulgeTarget,
  } as const
}

function snapshotBinding(
  snapshot: ProjectSnapshot,
): BeginnerSkeletonEndpointSnapshotBinding {
  return {
    projectInstanceId: snapshot.project_instance_id,
    projectId: snapshot.project_id,
    revision: snapshot.revision,
  }
}

function matchesSnapshotBinding(
  snapshot: ProjectSnapshot,
  binding: BeginnerSkeletonEndpointSnapshotBinding,
) {
  return snapshot.project_instance_id === binding.projectInstanceId
    && snapshot.project_id === binding.projectId
    && snapshot.revision === binding.revision
}

function matchesEndpointEpochIdentity(
  epoch: BeginnerSkeletonEndpointEpoch,
  projectInstanceId: ProjectSnapshot['project_instance_id'] | undefined,
  projectId: ProjectSnapshot['project_id'] | undefined,
  revision: ProjectSnapshot['revision'] | undefined,
) {
  return epoch.projectInstanceId === projectInstanceId
    && epoch.projectId === projectId
    && epoch.revision === revision
}

function matchesEndpointEpochSnapshot(
  epoch: BeginnerSkeletonEndpointEpoch,
  snapshot: ProjectSnapshot,
) {
  return matchesEndpointEpochIdentity(
    epoch,
    snapshot.project_instance_id,
    snapshot.project_id,
    snapshot.revision,
  )
}
