import { useEffect, useId, useRef, useState } from 'react'
import * as THREE from 'three'
import { OrbitControls } from 'three/addons/controls/OrbitControls.js'
import type { RgbaColor } from '../lib/coreClient'
import {
  collectFoldTreeDependentFaces,
  rerootFoldPreviewTree,
  resolveSingleFoldAnchor,
} from '../lib/foldPreviewAnchoring'
import {
  FOLD_PREVIEW_ANGLE_DRAG_MAPPING,
  createFoldPreviewAngleDragState,
  isFoldPreviewAngleDragScreenHit,
  reduceFoldPreviewAngleDrag,
  type FoldPreviewAngleDragEvent,
  type FoldPreviewAngleDragPointerType,
  type FoldPreviewAngleDragState,
} from '../lib/foldPreviewAngleDrag'
import {
  applyFoldPreviewCameraCommand,
  createFoldPreviewSelectionGesture,
  resolveFoldPreviewCameraCommand,
} from '../lib/foldPreviewCameraInteraction'
import {
  type FoldPreviewCollisionAdjacency,
} from '../lib/foldPreviewCollision'
import {
  summarizeFoldPreviewCollision,
  type FoldPreviewFaceCollisionSeverity,
} from '../lib/foldPreviewCollisionPresentation'
import {
  collisionBadgeClass,
  collisionBadgeText,
  collisionDataStatus,
  collisionPoseKey,
  collisionSummariesEqual,
  describeCollisionSummary,
  type CollisionSummary,
} from '../lib/foldPreviewCollisionView'
import {
  createFoldPreviewContinuousMotionRunner,
  type FoldPreviewContinuousMotionRunner,
  type FoldPreviewContinuousMotionRunnerState,
} from '../lib/foldPreviewContinuousMotionRunner'
import {
  describeFoldPreviewContinuousMotionDetail,
  type FoldPreviewMotionFaceLabel,
} from '../lib/foldPreviewContinuousMotionDetail'
import {
  describeFoldPreviewContinuousMotion,
} from '../lib/foldPreviewContinuousMotionView'
import { createFoldPreviewFaceGeometry } from '../lib/foldPreviewGeometry'
import type { FoldPreviewHingeContactConstraint } from '../lib/foldPreviewHingeCollision'
import {
  type FoldPreviewHingeAngle,
} from '../lib/foldPreviewKinematics'
import {
  createLatestFrameTask,
  type LatestFrameTask,
} from '../lib/latestFrameTask'
import type { FoldPreviewFaceModel, FoldPreviewModel } from '../lib/foldPreviewModel'
import { prepareFoldPreviewNarrowPhase } from '../lib/foldPreviewNarrowCollision'
import {
  FOLD_PREVIEW_PHYSICAL_GRAB_MAPPING,
  type FoldPreviewPhysicalGrabRay,
  type FoldPreviewPhysicalGrabSession,
} from '../lib/foldPreviewPhysicalGrab'
import {
  currentFoldPreviewPhysicalGrabGuardKey,
  planFoldPreviewPhysicalGrabTransition,
} from '../lib/foldPreviewPhysicalGrabCoordinator'
import {
  collectFoldPreviewPhysicalGrabPointerSamples,
  createFoldPreviewPhysicalGrabGestureState,
  reduceFoldPreviewPhysicalGrabGesture,
  type FoldPreviewPhysicalGrabGestureEvent,
  type FoldPreviewPhysicalGrabGestureState,
} from '../lib/foldPreviewPhysicalGrabGesture'
import {
  canBeginFoldPreviewPhysicalGrabInView,
  snapshotFoldPreviewPhysicalGrabView,
  type FoldPreviewPhysicalGrabViewport,
} from '../lib/foldPreviewPhysicalGrabView'
import {
  pickFoldPreviewFaceSurface,
  pickFoldPreviewTarget,
  type FoldPreviewPickObject,
  type FoldPreviewPickTarget,
} from '../lib/foldPreviewPicking'
import { calculateSingleFoldPose } from '../lib/foldPreviewSingleFoldKinematics'
import { prepareFoldPreviewSingleFoldPhysicalGrab } from '../lib/foldPreviewSingleFoldPhysicalGrab'
import {
  prepareFoldPreviewSingleFoldContinuousCollision,
  type FoldPreviewSingleFoldContinuousBlocker,
} from '../lib/foldPreviewSingleFoldContinuousCollision'
import {
  applyFoldPreviewTreeScenePose,
  createFoldPreviewTreeSceneCollisionPoseKey,
  lockFoldPreviewTreeSceneMatrixTarget,
} from '../lib/foldPreviewTreeScenePose'
import {
  prepareFoldPreviewTreeMotionContext,
} from '../lib/foldPreviewTreeMotionContext'
import {
  createFoldPreviewTreeMotionOwnerState,
  transitionFoldPreviewTreeMotionOwner,
  type FoldPreviewTreeMotionOwnerCommand,
  type FoldPreviewTreeMotionOwnerState,
} from '../lib/foldPreviewTreeMotionOwner'
import {
  createFoldPreviewTreeMotionRuntime,
  type FoldPreviewTreeMotionRuntimeState,
} from '../lib/foldPreviewTreeMotionRuntime'

type FoldPreviewProps = {
  angle: number
  hingeAngles?: readonly FoldPreviewHingeAngle[]
  selectedHingeId?: string | null
  fixedFaceId?: string | null
  onSelectHinge?: (edgeId: string | null) => void
  onChooseFixedFace?: (faceId: string) => void
  onRequestFoldAngle?: (angleDegrees: number) => void
  onCommitHingeFoldAngle?: (edgeId: string, angleDegrees: number) => void
  model?: FoldPreviewModel | null
  statusMessage?: string
  frontColor?: RgbaColor | null
  backColor?: RgbaColor | null
  thicknessMm?: number | null
}

type PreviewRuntime = {
  schedulePose: (angle: number, hingeAngles?: readonly FoldPreviewHingeAngle[]) => boolean
  updateSelection: (selectedHingeId: string | null) => void
  render: () => void
  cancelAngleDrag: () => void
  resetView: () => void
  dispose: () => void
}

type PendingPose = Readonly<{
  angle: number
  hingeAngles?: readonly FoldPreviewHingeAngle[]
  requestKey: string
}>

type PendingTreeDirectPose = Readonly<{
  angle: number
  hingeAngles: readonly FoldPreviewHingeAngle[]
  requestKey: string
  ownerToken: FoldPreviewTreeMotionOwnerState['ownerToken']
  generation: number
}>

type RenderedTreePoseSnapshot = Readonly<{
  model: FoldPreviewModel
  fixedFaceId: string
  collisionThickness: number | null
  visualThickness: number
  appliedAngles: readonly FoldPreviewHingeAngle[]
  requestKey: string
}>

type LatestRequestedTreePose = Readonly<{
  model: FoldPreviewModel
  fixedFaceId: string
  collisionThickness: number | null
  visualThickness: number
  requestKey: string
}>

type ContextualMotionState = Readonly<{
  contextKey: string
  state: FoldPreviewContinuousMotionRunnerState<FoldPreviewSingleFoldContinuousBlocker>
}>

type AngleDragPresentation = Readonly<{
  state: 'idle' | 'armed' | 'dragging'
  mapping: typeof FOLD_PREVIEW_ANGLE_DRAG_MAPPING
    | typeof FOLD_PREVIEW_PHYSICAL_GRAB_MAPPING
    | null
  pointerType: string | null
  captured: boolean
  startApplied: number | null
  target: number | null
  hingeId: string | null
  sequence: number
  cameraControlsEnabled: boolean
}>

const DEFAULT_THICKNESS_MM = 0.1
const MIN_VISIBLE_THICKNESS = 0.025
const MAX_VISIBLE_THICKNESS = 0.35
const MIN_ANGLE_DRAG_HINGE_LENGTH_CSS = 12
const MAX_ANGLE_DRAG_HINGE_DISTANCE_CSS = 12

const INITIAL_ANGLE_DRAG_PRESENTATION: AngleDragPresentation = Object.freeze({
  state: 'idle',
  mapping: null,
  pointerType: null,
  captured: false,
  startApplied: null,
  target: null,
  hingeId: null,
  sequence: 0,
  cameraControlsEnabled: true,
})

export function FoldPreview({
  angle,
  hingeAngles,
  selectedHingeId,
  fixedFaceId,
  onSelectHinge,
  onChooseFixedFace,
  onRequestFoldAngle,
  onCommitHingeFoldAngle,
  model,
  statusMessage,
  frontColor,
  backColor,
  thicknessMm,
}: FoldPreviewProps) {
  const hostRef = useRef<HTMLDivElement>(null)
  const runtimeRef = useRef<PreviewRuntime | null>(null)
  const descriptionId = useId()
  const [renderError, setRenderError] = useState<string | null>(null)
  const [collisionSummary, setCollisionSummary] = useState<CollisionSummary | null>(null)
  const [motionSnapshot, setMotionSnapshot] = useState<ContextualMotionState | null>(null)
  const [renderedTreePoseSnapshot, setRenderedTreePoseSnapshot] =
    useState<RenderedTreePoseSnapshot | null>(null)
  const [angleDragPresentation, setAngleDragPresentation] = useState(
    INITIAL_ANGLE_DRAG_PRESENTATION,
  )
  const angleDragSequenceRef = useRef(0)
  const motionSnapshotRef = useRef(motionSnapshot)
  motionSnapshotRef.current = motionSnapshot
  // Assignment selects the fold direction; the control supplies only its magnitude.
  const safeAngle = Number.isFinite(angle) ? THREE.MathUtils.clamp(angle, 0, 180) : 0
  const angleRef = useRef(safeAngle)
  angleRef.current = safeAngle
  const hingeAnglesRef = useRef(hingeAngles)
  hingeAnglesRef.current = hingeAngles
  const selectedHingeIdRef = useRef(selectedHingeId ?? null)
  selectedHingeIdRef.current = selectedHingeId ?? null
  const onSelectHingeRef = useRef(onSelectHinge)
  onSelectHingeRef.current = onSelectHinge
  const onChooseFixedFaceRef = useRef(onChooseFixedFace)
  onChooseFixedFaceRef.current = onChooseFixedFace
  const onRequestFoldAngleRef = useRef(onRequestFoldAngle)
  onRequestFoldAngleRef.current = onRequestFoldAngle
  const onCommitHingeFoldAngleRef = useRef(onCommitHingeFoldAngle)
  onCommitHingeFoldAngleRef.current = onCommitHingeFoldAngle
  const resolvedFixedFaceId = fixedFaceId
    ?? (model?.kind === 'single_fold'
      ? model.fixedFace.id
      : model?.kind === 'fold_graph' && model.kinematics.kind === 'tree'
        ? model.kinematics.rootFaceId
        : null)

  const hasAuthoritativeThickness = isNonNegativeFinite(thicknessMm)
  const safeThicknessMm = hasAuthoritativeThickness ? thicknessMm : DEFAULT_THICKNESS_MM
  const physicalPreviewThickness = model
    ? safeThicknessMm * model.worldUnitsPerMillimetre
    : 0
  const requestedCollisionThickness = model && hasAuthoritativeThickness
    ? thicknessMm * model.worldUnitsPerMillimetre
    : null
  const collisionThickness = isNonNegativeFinite(requestedCollisionThickness)
    ? requestedCollisionThickness
    : null
  const singleFoldMotionContextKey = model?.kind === 'single_fold'
    ? collisionPoseKey(
        model,
        resolvedFixedFaceId,
        collisionThickness,
        0,
        undefined,
      )
    : null
  const singleFoldMotionContextKeyRef = useRef(singleFoldMotionContextKey)
  singleFoldMotionContextKeyRef.current = singleFoldMotionContextKey
  const previewThickness = THREE.MathUtils.clamp(
    physicalPreviewThickness,
    MIN_VISIBLE_THICKNESS,
    MAX_VISIBLE_THICKNESS,
  )
  const { hex: frontHex, opacity: frontOpacity } = resolveColor(frontColor, 0xf5a65b)
  const { hex: backHex, opacity: backOpacity } = resolveColor(backColor, 0xfffdf9)
  const thicknessIsEmphasised = physicalPreviewThickness < MIN_VISIBLE_THICKNESS
  const thicknessIsLimited = physicalPreviewThickness > MAX_VISIBLE_THICKNESS
  const latestRequestedTreePose = (() => {
    if (
      model?.kind !== 'fold_graph'
      || model.kinematics.kind !== 'tree'
    ) return null
    const fixedFaceId =
      resolvedFixedFaceId ?? model.kinematics.rootFaceId
    const requestedAngles = hingeAngles
      ?? model.kinematics.joints.map((joint) => ({
        edgeId: joint.hinge.edgeId,
        angleDegrees: safeAngle,
      }))
    const requestKey = createFoldPreviewTreeSceneCollisionPoseKey(
      model,
      fixedFaceId,
      collisionThickness,
      requestedAngles,
    )
    return requestKey
      ? {
          model,
          fixedFaceId,
          collisionThickness,
          visualThickness: previewThickness,
          requestKey,
        }
      : null
  })()
  const latestRequestedTreePoseRef =
    useRef<LatestRequestedTreePose | null>(latestRequestedTreePose)
  latestRequestedTreePoseRef.current = latestRequestedTreePose

  useEffect(() => {
    const host = hostRef.current
    if (!host || !model) {
      runtimeRef.current = null
      setCollisionSummary(null)
      setRenderedTreePoseSnapshot(null)
      return
    }
    setRenderError(null)
    setCollisionSummary(null)
    setRenderedTreePoseSnapshot(null)

    const singleAnchor = model.kind === 'single_fold'
      ? resolveSingleFoldAnchor(model, resolvedFixedFaceId ?? model.fixedFace.id)
      : null
    const treeKinematics = model.kind === 'fold_graph' && model.kinematics.kind === 'tree'
      ? rerootFoldPreviewTree(model.kinematics, resolvedFixedFaceId ?? model.kinematics.rootFaceId)
      : null
    if (
      (model.kind === 'single_fold' && !singleAnchor)
      || (model.kind === 'fold_graph' && model.kinematics.kind === 'tree' && !treeKinematics)
    ) {
      runtimeRef.current = null
      setRenderError('固定面を安全に解決できませんでした')
      return
    }

    const geometries: THREE.BufferGeometry[] = []
    const edgeGeometries: THREE.EdgesGeometry[] = []
    const hingeGeometries: THREE.BufferGeometry[] = []
    const staticFaces: Array<{
      face: FoldPreviewFaceModel
      geometry: THREE.BufferGeometry
    }> = []
    let movingGeometry: THREE.BufferGeometry | null = null
    try {
      if (model.kind === 'single_fold') {
        if (!singleAnchor) throw new Error('missing single-fold anchor')
        const fixedGeometry = createFoldPreviewFaceGeometry(
          singleAnchor.fixedFace.polygon,
          previewThickness,
        )
        geometries.push(fixedGeometry)
        staticFaces.push({ face: singleAnchor.fixedFace, geometry: fixedGeometry })

        const { start } = model.hinge
        movingGeometry = createFoldPreviewFaceGeometry(
          singleAnchor.movingFace.polygon.map((point) => ({
            x: point.x - start.x,
            z: point.z - start.z,
          })),
          previewThickness,
        )
        geometries.push(movingGeometry)
      } else {
        for (const face of model.faces) {
          const geometry = createFoldPreviewFaceGeometry(face.polygon, previewThickness)
          geometries.push(geometry)
          staticFaces.push({ face, geometry })
        }
      }
    } catch {
      for (const geometry of geometries) attemptCleanup(() => geometry.dispose())
      setRenderError('3D面を安全に三角形化できませんでした')
      return
    }

    let renderer: THREE.WebGLRenderer | null = null
    let grid: THREE.GridHelper | null = null
    let frontMaterial: THREE.MeshStandardMaterial | null = null
    let backMaterial: THREE.MeshStandardMaterial | null = null
    let sideMaterial: THREE.MeshStandardMaterial | null = null
    let edgeMaterial: THREE.LineBasicMaterial | null = null
    let fixedFaceEdgeMaterial: THREE.LineBasicMaterial | null = null
    let dependentFaceEdgeMaterial: THREE.LineBasicMaterial | null = null
    let collisionContactEdgeMaterial: THREE.LineBasicMaterial | null = null
    let collisionIndeterminateEdgeMaterial: THREE.LineBasicMaterial | null = null
    let collisionPenetrationEdgeMaterial: THREE.LineBasicMaterial | null = null
    let hingeMaterial: THREE.LineBasicMaterial | null = null
    let selectedHingeMaterial: THREE.LineBasicMaterial | null = null
    let observer: ResizeObserver | null = null
    let controls: OrbitControls | null = null
    let controlsChangeHandler: (() => void) | null = null
    let pointerDownHandler: ((event: PointerEvent) => void) | null = null
    let pointerMoveHandler: ((event: PointerEvent) => void) | null = null
    let pointerUpHandler: ((event: PointerEvent) => void) | null = null
    let pointerCancelHandler: ((event: PointerEvent) => void) | null = null
    let lostPointerCaptureHandler: ((event: PointerEvent) => void) | null = null
    let windowBlurHandler: (() => void) | null = null
    let keyDownHandler: ((event: KeyboardEvent) => void) | null = null
    let runtime: PreviewRuntime | null = null
    let poseFrameTask: LatestFrameTask<PendingPose> | null = null
    let treeDirectPoseFrameTask:
      LatestFrameTask<PendingTreeDirectPose> | null = null
    let treeMotionOwnerState:
      FoldPreviewTreeMotionOwnerState | null = null
    let treeMotionRuntimeState:
      FoldPreviewTreeMotionRuntimeState | null = null
    let renderedTreeAngles: readonly FoldPreviewHingeAngle[] | null = null
    let renderedTreePoseKey: string | null = null
    let selectedTreeHingeId = selectedHingeIdRef.current
    let resetTreeOwnedGesture = () => undefined
    let continuousMotionRunner:
      FoldPreviewContinuousMotionRunner<FoldPreviewSingleFoldContinuousBlocker> | null = null
    let angleDragState: FoldPreviewAngleDragState = createFoldPreviewAngleDragState()
    let physicalGrabState: FoldPreviewPhysicalGrabGestureState =
      createFoldPreviewPhysicalGrabGestureState()
    let physicalGrabStartRunnerState:
      FoldPreviewContinuousMotionRunnerState<FoldPreviewSingleFoldContinuousBlocker>
      | null = null
    let physicalGrabCameraSnapshot: string | null = null
    let physicalGrabGuardKey: string | null = null
    let physicalGrabSessionForEvents: FoldPreviewPhysicalGrabSession | null = null
    let physicalGrabGuardSequence = 0
    let angleDragHingeId: string | null = null
    let angleDragCapturedPointerId: number | null = null
    let angleDragCapturedPointerType: FoldPreviewAngleDragPointerType | null = null
    let angleDragContextKey: string | null = null
    let angleDragFrameHandle: number | null = null
    let hasPendingAngleDragTarget = false
    let controlsEnabledBeforeAngleDrag: boolean | null = null
    let cursorBeforeAngleDrag: string | null = null
    const activeDocumentPointerIds = new Set<number>()
    let disposed = false
    const hinges = model.kind === 'single_fold'
      ? [model.hinge]
      : model.kind === 'fold_graph'
        ? model.hinges
        : []
    const collisionAdjacencies: FoldPreviewCollisionAdjacency[] = hinges.map((hinge) => ({
      edgeId: hinge.edgeId,
      firstFaceId: hinge.leftFaceId,
      secondFaceId: hinge.rightFaceId,
    }))
    const collisionHingeConstraints: FoldPreviewHingeContactConstraint[] = hinges.map(
      (hinge) => ({
        edgeId: hinge.edgeId,
        leftFaceId: hinge.leftFaceId,
        rightFaceId: hinge.rightFaceId,
        start: {
          vertexId: hinge.start.vertexId,
          x: hinge.start.x,
          z: hinge.start.z,
        },
        end: {
          vertexId: hinge.end.vertexId,
          x: hinge.end.x,
          z: hinge.end.z,
        },
        thicknessRule: 'centered_mid_surface_v1',
      }),
    )
    const collisionAnalyzer = (() => {
      try {
        return prepareFoldPreviewNarrowPhase(
          model.faces,
          collisionAdjacencies,
          collisionHingeConstraints,
        )
      } catch {
        return null
      }
    })()
    const singleFoldContinuousAnalyzer = (() => {
      if (model.kind !== 'single_fold') return null
      try {
        return prepareFoldPreviewSingleFoldContinuousCollision(
          model,
          resolvedFixedFaceId ?? model.fixedFace.id,
        )
      } catch {
        return null
      }
    })()
    let collisionSeverityByFace = new Map<string, FoldPreviewFaceCollisionSeverity>()
    let refreshFaceHighlights = () => undefined

    const updateCollision = (
      faceTransforms: ReadonlyMap<string, THREE.Matrix4>,
      requestKey: string,
    ) => {
      let nextSummary: CollisionSummary = { kind: 'unavailable', requestKey }
      let nextCollisionSeverityByFace = new Map<string, FoldPreviewFaceCollisionSeverity>()
      try {
        const result = collisionThickness === null
          ? null
          : collisionAnalyzer?.analyze(faceTransforms, collisionThickness)
        if (result) {
          const presentation = summarizeFoldPreviewCollision(result)
          nextCollisionSeverityByFace = new Map(presentation.faceSeverities)
          nextSummary = {
            kind: 'ready',
            requestKey,
            totalCandidates: presentation.totalCandidates,
            nonAdjacentCandidates: presentation.nonAdjacentCandidates,
            hingeAdjacentCandidates: presentation.hingeAdjacentCandidates,
            narrowInteractions: presentation.narrowInteractions,
            nonAdjacentPenetrations: presentation.nonAdjacentPenetrations,
            nonAdjacentContacts: presentation.nonAdjacentContacts,
            hingeInteractions: presentation.hingeInteractions,
            hingeModelAllowedContacts: presentation.hingeModelAllowedContacts,
            hingeModelCorridorOverlaps: presentation.hingeModelCorridorOverlaps,
            hingeOutsidePenetrations: presentation.hingeOutsidePenetrations,
            hingeOutsideContacts: presentation.hingeOutsideContacts,
            hingeUnresolvedInteractions: presentation.hingeUnresolvedInteractions,
            indeterminateInteractions: presentation.indeterminateInteractions,
          }
        }
      } catch {
        // Collision diagnostics are optional and must not invalidate a verified pose.
      }
      if (!disposed) {
        collisionSeverityByFace = nextCollisionSeverityByFace
        try {
          refreshFaceHighlights()
        } catch {
          collisionSeverityByFace = new Map()
          try {
            refreshFaceHighlights()
          } catch {
            // Highlight recovery is best-effort and must not break the 3D preview.
          }
        }
        setCollisionSummary((current) =>
          collisionSummariesEqual(current, nextSummary) ? current : nextSummary)
      }
    }

    const executeTreeOwnershipCleanupCommand = (
      command: FoldPreviewTreeMotionOwnerCommand,
    ) => {
      switch (command.kind) {
        case 'reset_gesture':
          resetTreeOwnedGesture()
          return true
        case 'dispose_runner':
          // Runtime callbacks always consult the current state reference.
          // Dropping this snapshot makes callbacks from the prior generation inert.
          treeMotionRuntimeState = null
          return true
        case 'dispose_direct':
          treeDirectPoseFrameTask?.dispose()
          treeDirectPoseFrameTask = null
          return true
        default:
          return false
      }
    }

    const dispose = () => {
      if (disposed) return
      if (treeMotionOwnerState) {
        const ownerPlan = transitionFoldPreviewTreeMotionOwner(
          treeMotionOwnerState,
          { kind: 'dispose' },
        )
        if (ownerPlan) {
          treeMotionOwnerState = ownerPlan.state
          for (const command of ownerPlan.commands) {
            executeTreeOwnershipCleanupCommand(command)
          }
        }
      }
      disposed = true
      angleDragState = reduceFoldPreviewAngleDrag(angleDragState, {
        kind: 'reset',
        reason: 'dispose',
      }).state
      physicalGrabState = reduceFoldPreviewPhysicalGrabGesture(
        physicalGrabState,
        { kind: 'reset', reason: 'dispose' },
      ).state
      physicalGrabStartRunnerState = null
      physicalGrabCameraSnapshot = null
      physicalGrabGuardKey = null
      physicalGrabSessionForEvents = null
      if (angleDragFrameHandle !== null) {
        attemptCleanup(() => window.cancelAnimationFrame(angleDragFrameHandle!))
        angleDragFrameHandle = null
      }
      hasPendingAngleDragTarget = false
      if (renderer) {
        const canvas = renderer.domElement
        if (pointerDownHandler) {
          attemptCleanup(() =>
            canvas.ownerDocument.removeEventListener(
              'pointerdown',
              pointerDownHandler!,
              true,
            ))
        }
        if (pointerMoveHandler) {
          attemptCleanup(() =>
            canvas.ownerDocument.removeEventListener('pointermove', pointerMoveHandler!, true))
        }
        if (pointerUpHandler) {
          attemptCleanup(() =>
            canvas.ownerDocument.removeEventListener('pointerup', pointerUpHandler!, true))
        }
        if (pointerCancelHandler) {
          attemptCleanup(() =>
            canvas.ownerDocument.removeEventListener(
              'pointercancel',
              pointerCancelHandler!,
              true,
            ))
        }
        if (lostPointerCaptureHandler) {
          attemptCleanup(() =>
            canvas.removeEventListener('lostpointercapture', lostPointerCaptureHandler!))
        }
        if (
          angleDragCapturedPointerId !== null
          && canvas.hasPointerCapture(angleDragCapturedPointerId)
        ) {
          attemptCleanup(() => canvas.releasePointerCapture(angleDragCapturedPointerId!))
        }
      }
      angleDragCapturedPointerId = null
      angleDragCapturedPointerType = null
      angleDragHingeId = null
      angleDragContextKey = null
      if (windowBlurHandler) {
        attemptCleanup(() => window.removeEventListener('blur', windowBlurHandler!))
      }
      if (controls && controlsEnabledBeforeAngleDrag !== null) {
        controls.enabled = controlsEnabledBeforeAngleDrag
      }
      controlsEnabledBeforeAngleDrag = null
      if (renderer && cursorBeforeAngleDrag !== null) {
        renderer.domElement.style.cursor = cursorBeforeAngleDrag
      }
      cursorBeforeAngleDrag = null
      activeDocumentPointerIds.clear()
      setAngleDragPresentation((current) => current.state === 'idle'
        && current.cameraControlsEnabled
        ? current
        : {
            ...INITIAL_ANGLE_DRAG_PRESENTATION,
            sequence: current.sequence,
          })
      if (keyDownHandler) {
        attemptCleanup(() => host.removeEventListener('keydown', keyDownHandler!))
      }
      if (controls && controlsChangeHandler) {
        attemptCleanup(() => controls?.removeEventListener('change', controlsChangeHandler!))
      }
      attemptCleanup(() => controls?.dispose())
      attemptCleanup(() => observer?.disconnect())
      attemptCleanup(() => poseFrameTask?.dispose())
      attemptCleanup(() => treeDirectPoseFrameTask?.dispose())
      attemptCleanup(() => continuousMotionRunner?.dispose())
      if (runtime && runtimeRef.current === runtime) runtimeRef.current = null
      for (const geometry of geometries) attemptCleanup(() => geometry.dispose())
      for (const geometry of edgeGeometries) attemptCleanup(() => geometry.dispose())
      for (const geometry of hingeGeometries) attemptCleanup(() => geometry.dispose())
      if (grid) {
        attemptCleanup(() => grid?.geometry.dispose())
        attemptCleanup(() => disposeMaterial(grid?.material ?? []))
      }
      attemptCleanup(() => frontMaterial?.dispose())
      attemptCleanup(() => backMaterial?.dispose())
      attemptCleanup(() => sideMaterial?.dispose())
      attemptCleanup(() => edgeMaterial?.dispose())
      attemptCleanup(() => fixedFaceEdgeMaterial?.dispose())
      attemptCleanup(() => dependentFaceEdgeMaterial?.dispose())
      attemptCleanup(() => collisionContactEdgeMaterial?.dispose())
      attemptCleanup(() => collisionIndeterminateEdgeMaterial?.dispose())
      attemptCleanup(() => collisionPenetrationEdgeMaterial?.dispose())
      attemptCleanup(() => hingeMaterial?.dispose())
      attemptCleanup(() => selectedHingeMaterial?.dispose())
      if (renderer) {
        attemptCleanup(() => renderer?.renderLists.dispose())
        attemptCleanup(() => renderer?.dispose())
        attemptCleanup(() => renderer?.domElement.remove())
      }
    }

    try {
      const scene = new THREE.Scene()
      scene.background = new THREE.Color('#eef2f5')
      const initialSize = readRenderableSize(host)
      const camera = new THREE.PerspectiveCamera(
        36,
        initialSize ? initialSize.width / initialSize.height : 1,
        0.1,
        100,
      )
      camera.position.set(5.4, 4.7, 6.4)
      camera.lookAt(0, 0, 0)

      const createdRenderer = new THREE.WebGLRenderer({ antialias: true, alpha: false })
      renderer = createdRenderer
      const devicePixelRatio = Number.isFinite(window.devicePixelRatio) && window.devicePixelRatio > 0
        ? window.devicePixelRatio
        : 1
      createdRenderer.setPixelRatio(Math.min(devicePixelRatio, 2))
      createdRenderer.setSize(initialSize?.width ?? 1, initialSize?.height ?? 1, false)
      createdRenderer.outputColorSpace = THREE.SRGBColorSpace
      createdRenderer.shadowMap.enabled = true
      createdRenderer.shadowMap.type = THREE.PCFSoftShadowMap
      createdRenderer.domElement.setAttribute('aria-hidden', 'true')
      host.appendChild(createdRenderer.domElement)

      scene.add(new THREE.HemisphereLight(0xffffff, 0x748090, 2.2))
      const light = new THREE.DirectionalLight(0xffffff, 2.5)
      light.position.set(3, 7, 4)
      light.castShadow = true
      scene.add(light)

      const createdGrid = new THREE.GridHelper(8, 16, 0xb8c1cc, 0xd7dde4)
      grid = createdGrid
      createdGrid.position.y = -1.35
      scene.add(createdGrid)

      const createdFrontMaterial = createPaperMaterial({ hex: frontHex, opacity: frontOpacity })
      frontMaterial = createdFrontMaterial
      const createdBackMaterial = createPaperMaterial({ hex: backHex, opacity: backOpacity })
      backMaterial = createdBackMaterial
      const createdSideMaterial = new THREE.MeshStandardMaterial({
        color: mixColors(frontHex, backHex),
        roughness: 0.82,
      })
      sideMaterial = createdSideMaterial
      const materials = [createdFrontMaterial, createdBackMaterial, createdSideMaterial]
      const createdEdgeMaterial = new THREE.LineBasicMaterial({ color: 0x715747 })
      edgeMaterial = createdEdgeMaterial
      const createdFixedFaceEdgeMaterial = new THREE.LineBasicMaterial({
        color: 0x1671b8,
        depthTest: false,
        depthWrite: false,
      })
      fixedFaceEdgeMaterial = createdFixedFaceEdgeMaterial
      const createdDependentFaceEdgeMaterial = new THREE.LineBasicMaterial({
        color: 0xe24a16,
        depthTest: false,
        depthWrite: false,
      })
      dependentFaceEdgeMaterial = createdDependentFaceEdgeMaterial
      const createdCollisionContactEdgeMaterial = new THREE.LineBasicMaterial({
        color: 0x8e44ad,
        depthTest: false,
        depthWrite: false,
      })
      collisionContactEdgeMaterial = createdCollisionContactEdgeMaterial
      const createdCollisionIndeterminateEdgeMaterial = new THREE.LineBasicMaterial({
        color: 0xb18412,
        depthTest: false,
        depthWrite: false,
      })
      collisionIndeterminateEdgeMaterial = createdCollisionIndeterminateEdgeMaterial
      const createdCollisionPenetrationEdgeMaterial = new THREE.LineBasicMaterial({
        color: 0xc62828,
        depthTest: false,
        depthWrite: false,
      })
      collisionPenetrationEdgeMaterial = createdCollisionPenetrationEdgeMaterial

      const faceEdgeLines = new Map<string, THREE.LineSegments>()
      let dependentFaceIdsForHighlight = new Set<string>()
      refreshFaceHighlights = () => {
        for (const [faceId, line] of faceEdgeLines) {
          const fixed = faceId === resolvedFixedFaceId
          const dependent = dependentFaceIdsForHighlight.has(faceId)
          const collisionSeverity = collisionSeverityByFace.get(faceId)
          line.material = collisionSeverity === 'penetrating'
            ? createdCollisionPenetrationEdgeMaterial
            : collisionSeverity === 'indeterminate'
              ? createdCollisionIndeterminateEdgeMaterial
              : collisionSeverity === 'contact'
                ? createdCollisionContactEdgeMaterial
                : fixed
                  ? createdFixedFaceEdgeMaterial
                  : dependent
                    ? createdDependentFaceEdgeMaterial
                    : createdEdgeMaterial
          line.renderOrder = collisionSeverity === 'penetrating'
            ? 13
            : collisionSeverity === 'indeterminate'
              ? 12
              : collisionSeverity === 'contact'
                ? 11
                : fixed ? 9 : dependent ? 8 : 0
        }
      }
      const facePickObjects: FoldPreviewPickObject[] = []
      const makeFace = (geometry: THREE.BufferGeometry, face: FoldPreviewFaceModel) => {
        const group = new THREE.Group()
        group.userData.faceId = face.id
        const paper = new THREE.Mesh(geometry, materials)
        paper.userData.faceId = face.id
        facePickObjects.push({ id: face.id, object: paper })
        paper.castShadow = true
        paper.receiveShadow = true
        const edgeGeometry = new THREE.EdgesGeometry(geometry, 20)
        edgeGeometries.push(edgeGeometry)
        const faceEdges = new THREE.LineSegments(edgeGeometry, createdEdgeMaterial)
        faceEdgeLines.set(face.id, faceEdges)
        group.add(paper, faceEdges)
        return group
      }

      const faceGroups = new Map<string, THREE.Group>()
      for (const { face, geometry } of staticFaces) {
        const group = makeFace(geometry, face)
        if (model.kind === 'fold_graph') {
          group.matrixAutoUpdate = false
          if (
            model.kinematics.kind === 'tree'
            && !lockFoldPreviewTreeSceneMatrixTarget(group.matrix)
          ) throw new Error('tree face matrix registration failed')
          faceGroups.set(face.id, group)
        }
        scene.add(group)
      }

      let pivot: THREE.Group | null = null
      let axis: THREE.Vector3 | null = null
      let rotationSign: 1 | -1 = 1
      let initialPoseAngle = angleRef.current
      let updatePose = (_angle: number, _hingeAngles?: readonly FoldPreviewHingeAngle[]) => true
      let updateSelection = (_selectedHingeId: string | null) => undefined
      if (model.kind === 'single_fold' && movingGeometry) {
        if (!singleAnchor) throw new Error('missing single-fold anchor')
        pivot = new THREE.Group()
        pivot.position.set(model.hinge.start.x, 0, model.hinge.start.z)
        pivot.add(makeFace(movingGeometry, singleAnchor.movingFace))
        axis = new THREE.Vector3(
          model.hinge.end.x - model.hinge.start.x,
          0,
          model.hinge.end.z - model.hinge.start.z,
        ).normalize()
        rotationSign = singleAnchor.movingRotationSign
        scene.add(pivot)
        const preservedMotion = motionSnapshotRef.current
        initialPoseAngle = preservedMotion?.contextKey === singleFoldMotionContextKey
          && isFoldPreviewAngle(preservedMotion.state.applied)
          ? preservedMotion.state.applied
          : 0
        updatePose = (nextAngle) => {
          if (!pivot || !axis) return false
          const pose = calculateSingleFoldPose(
            model,
            resolvedFixedFaceId ?? model.fixedFace.id,
            nextAngle,
          )
          if (
            !pose
            || pose.fixedFaceId !== singleAnchor.fixedFace.id
            || pose.movingFaceId !== singleAnchor.movingFace.id
          ) return false
          applyFoldRotation(pivot, axis, rotationSign, nextAngle)
          updateCollision(pose.faceTransforms, collisionPoseKey(
            model,
            resolvedFixedFaceId,
            collisionThickness,
            nextAngle,
            undefined,
          ))
          return true
        }
        if (!updatePose(initialPoseAngle)) throw new Error('invalid single-fold pose')
      }

      const hingePickObjects: FoldPreviewPickObject[] = []
      if (hinges.length > 0) {
        const createdHingeMaterial = new THREE.LineBasicMaterial({ color: 0x7a3f16 })
        hingeMaterial = createdHingeMaterial
        const createdSelectedHingeMaterial = new THREE.LineBasicMaterial({
          color: 0xe24a16,
          depthTest: false,
          depthWrite: false,
        })
        selectedHingeMaterial = createdSelectedHingeMaterial
        const hingeLines = new Map<string, THREE.LineSegments>()
        for (const hinge of hinges) {
          const geometry = new THREE.BufferGeometry()
          hingeGeometries.push(geometry)
          geometry.setFromPoints([
            new THREE.Vector3(
              hinge.start.x,
              previewThickness / 2 + 0.008,
              hinge.start.z,
            ),
            new THREE.Vector3(
              hinge.end.x,
              previewThickness / 2 + 0.008,
              hinge.end.z,
            ),
          ])
          const line = new THREE.LineSegments(geometry, createdHingeMaterial)
          line.userData.hingeId = hinge.edgeId
          hingePickObjects.push({ id: hinge.edgeId, object: line })
          hingeLines.set(hinge.edgeId, line)
          if (model.kind === 'fold_graph' && model.kinematics.kind === 'tree') {
            line.matrixAutoUpdate = false
            if (!lockFoldPreviewTreeSceneMatrixTarget(line.matrix)) {
              throw new Error('tree hinge matrix registration failed')
            }
          }
          scene.add(line)
        }

        updateSelection = (nextSelectedHingeId) => {
          for (const [edgeId, line] of hingeLines) {
            const selected = edgeId === nextSelectedHingeId
            line.material = selected ? createdSelectedHingeMaterial : createdHingeMaterial
            line.renderOrder = selected ? 10 : 0
          }
          const dependentFaceIds = new Set<string>()
          if (
            nextSelectedHingeId
            && model.kind === 'single_fold'
            && singleAnchor
            && nextSelectedHingeId === model.hinge.edgeId
          ) {
            dependentFaceIds.add(singleAnchor.movingFace.id)
          } else if (
            nextSelectedHingeId
            && model.kind === 'fold_graph'
            && model.kinematics.kind === 'tree'
            && treeKinematics
          ) {
            const resolvedDependentFaceIds = collectFoldTreeDependentFaces(
              treeKinematics,
              nextSelectedHingeId,
            )
            if (!resolvedDependentFaceIds) throw new Error('invalid dependent face tree')
            for (const faceId of resolvedDependentFaceIds) dependentFaceIds.add(faceId)
          }
          dependentFaceIdsForHighlight = dependentFaceIds
          refreshFaceHighlights()
        }
        updateSelection(selectedHingeIdRef.current)

        if (model.kind === 'fold_graph' && model.kinematics.kind === 'tree') {
          if (!treeKinematics) throw new Error('missing fold-tree anchor')
          const faceMatrixTargets = new Map(
            [...faceGroups].map(([faceId, group]) => [
              faceId,
              group.matrix,
            ]),
          )
          const hingeMatrixTargets = new Map(
            [...hingeLines].map(([edgeId, line]) => [
              edgeId,
              line.matrix,
            ]),
          )
          updatePose = (nextAngle, nextHingeAngles) => {
            const requestedAngles = nextHingeAngles
              ? nextHingeAngles.map((hingeAngle) => ({
                  edgeId: hingeAngle.edgeId,
                  angleDegrees: hingeAngle.angleDegrees,
                }))
              : treeKinematics.joints.map((joint) => ({
                  edgeId: joint.hinge.edgeId,
                  angleDegrees: nextAngle,
                }))
            const requestKey = createFoldPreviewTreeSceneCollisionPoseKey(
              model,
              treeKinematics.rootFaceId,
              collisionThickness,
              requestedAngles,
            )
            if (!requestKey) return false
            const application = applyFoldPreviewTreeScenePose({
              tree: treeKinematics,
              appliedAngles: requestedAngles,
              faceTargets: faceMatrixTargets,
              hingeTargets: hingeMatrixTargets,
            })
            if (!application) return false
            const confirmedAngles = application.appliedAngles.map(
              (hingeAngle) => ({
                edgeId: hingeAngle.edgeId,
                angleDegrees: hingeAngle.angleDegrees,
              }),
            )
            for (const group of faceGroups.values()) {
              group.matrixWorldNeedsUpdate = true
            }
            for (const line of hingeLines.values()) {
              line.matrixWorldNeedsUpdate = true
            }
            renderedTreeAngles = confirmedAngles
            renderedTreePoseKey = requestKey
            setRenderedTreePoseSnapshot({
              model,
              fixedFaceId: treeKinematics.rootFaceId,
              collisionThickness,
              visualThickness: previewThickness,
              appliedAngles: confirmedAngles,
              requestKey,
            })
            updateCollision(application.faceTransforms, requestKey)
            return true
          }
          if (!updatePose(angleRef.current, hingeAnglesRef.current)) {
            throw new Error('invalid fold tree pose')
          }
        }
      }
      if (
        model.kind === 'planar'
        || (model.kind === 'fold_graph' && model.kinematics.kind === 'static_cycle')
      ) {
        const flatFaceTransforms = new Map(
          model.faces.map((face) => [face.id, new THREE.Matrix4()]),
        )
        updateCollision(
          flatFaceTransforms,
          collisionPoseKey(
            model,
            resolvedFixedFaceId,
            collisionThickness,
            angleRef.current,
            hingeAnglesRef.current,
          ),
        )
        updatePose = (nextAngle, nextHingeAngles) => {
          updateCollision(
            flatFaceTransforms,
            collisionPoseKey(
              model,
              resolvedFixedFaceId,
              collisionThickness,
              nextAngle,
              nextHingeAngles,
            ),
          )
          return true
        }
      }

      const render = () => createdRenderer.render(scene, camera)
      const createdControls = new OrbitControls(camera, createdRenderer.domElement)
      controls = createdControls
      createdControls.target.set(0, 0, 0)
      createdControls.enableDamping = false
      createdControls.enableRotate = true
      createdControls.enableZoom = true
      createdControls.enablePan = true
      createdControls.screenSpacePanning = true
      createdControls.minDistance = 1
      createdControls.maxDistance = 40
      createdControls.minPolarAngle = 0.02
      createdControls.maxPolarAngle = Math.PI - 0.02
      createdControls.cursorStyle = 'grab'
      createdControls.update()
      createdControls.saveState()
      controlsChangeHandler = () => {
        if (disposed) return
        try {
          render()
        } catch {
          dispose()
          setRenderError('3Dカメラ操作を安全に継続できませんでした')
        }
      }
      createdControls.addEventListener('change', controlsChangeHandler)
      if (
        typeof window.requestAnimationFrame !== 'function'
        || typeof window.cancelAnimationFrame !== 'function'
      ) throw new Error('animation frame scheduling is unavailable')
      let schedulePose: PreviewRuntime['schedulePose']
      if (model.kind === 'single_fold') {
        if (!singleFoldMotionContextKey) {
          throw new Error('missing single-fold motion context')
        }
        const createdContinuousMotionRunner = createFoldPreviewContinuousMotionRunner({
          initialAngle: initialPoseAngle,
          schedule: (callback) => window.requestAnimationFrame(callback),
          cancel: (handle) => window.cancelAnimationFrame(handle),
          jobFactory: (startAngle, targetAngle) => collisionThickness === null
            ? null
            : singleFoldContinuousAnalyzer?.createJob(
                startAngle,
                targetAngle,
                collisionThickness,
              ) ?? null,
          applyAngle: (certifiedAngle) => {
            if (!updatePose(certifiedAngle)) return false
            render()
            return true
          },
          onState: (state) => {
            if (disposed) return
            if (
              state.status === 'indeterminate'
              && (
                state.reason === 'apply_angle_error'
                || state.reason === 'apply_angle_rejected'
              )
            ) {
              dispose()
              setRenderError('3D描画を安全に継続できませんでした')
              return
            }
            setMotionSnapshot({
              contextKey: singleFoldMotionContextKey,
              state,
            })
          },
        })
        if (!createdContinuousMotionRunner) {
          throw new Error('continuous motion runner is unavailable')
        }
        continuousMotionRunner = createdContinuousMotionRunner
        setMotionSnapshot({
          contextKey: singleFoldMotionContextKey,
          state: createdContinuousMotionRunner.getState(),
        })
        schedulePose = (nextAngle) => {
          const accepted = createdContinuousMotionRunner.request(nextAngle)
          return accepted
            || createdContinuousMotionRunner.getState().status === 'indeterminate'
        }
      } else if (
        model.kind === 'fold_graph'
        && model.kinematics.kind === 'tree'
        && treeKinematics
      ) {
        const requestedTreePose = (
          requestedAngle: number,
          requestedHingeAngles?: readonly FoldPreviewHingeAngle[],
        ) => {
          try {
            const completeAngles = requestedHingeAngles
              ? requestedHingeAngles.map((hingeAngle) => ({
                  edgeId: hingeAngle.edgeId,
                  angleDegrees: hingeAngle.angleDegrees,
                }))
              : treeKinematics.joints.map((joint) => ({
                  edgeId: joint.hinge.edgeId,
                  angleDegrees: requestedAngle,
                }))
            const requestKey = createFoldPreviewTreeSceneCollisionPoseKey(
              model,
              treeKinematics.rootFaceId,
              collisionThickness,
              completeAngles,
            )
            return requestKey
              ? {
                  angle: requestedAngle,
                  hingeAngles: completeAngles,
                  requestKey,
                }
              : null
          } catch {
            return null
          }
        }
        const createIdleTreeOwner = () => {
          const ownerState = createFoldPreviewTreeMotionOwnerState()
          if (!ownerState) return false
          treeMotionOwnerState = ownerState
          treeMotionRuntimeState = null
          return true
        }
        const resetTreeOwnerToIdle = () => {
          if (treeMotionOwnerState) {
            const ownerPlan = transitionFoldPreviewTreeMotionOwner(
              treeMotionOwnerState,
              { kind: 'dispose' },
            )
            if (!ownerPlan?.accepted) return false
            treeMotionOwnerState = ownerPlan.state
            for (const command of ownerPlan.commands) {
              if (!executeTreeOwnershipCleanupCommand(command)) {
                return false
              }
            }
          }
          return createIdleTreeOwner()
        }
        const prepareTreeMotionRuntime = (hingeEdgeId: string | null) => {
          if (
            !renderedTreeAngles
            || renderedTreePoseKey === null
            || collisionThickness === null
            || !hingeEdgeId
            || !treeKinematics.joints.some(
              (joint) => joint.hinge.edgeId === hingeEdgeId,
            )
          ) {
            if (
              treeMotionRuntimeState
              || treeMotionOwnerState?.owner === 'runner'
            ) return resetTreeOwnerToIdle()
            return treeMotionOwnerState ? true : createIdleTreeOwner()
          }
          if (
            treeMotionRuntimeState
            && treeMotionRuntimeState.hingeEdgeId === hingeEdgeId
            && treeMotionRuntimeState.appliedAngles.length
              === renderedTreeAngles.length
            && treeMotionRuntimeState.appliedAngles.every(
              (hingeAngle, index) =>
                hingeAngle.edgeId === renderedTreeAngles?.[index]?.edgeId
                && hingeAngle.angleDegrees
                  === renderedTreeAngles[index]?.angleDegrees,
            )
          ) return true
          if (!treeMotionOwnerState && !createIdleTreeOwner()) return false
          const ownerState = treeMotionOwnerState
          if (!ownerState) return false
          if (ownerState.directPending) return true
          const context = prepareFoldPreviewTreeMotionContext({
            model,
            fixedFaceId: treeKinematics.rootFaceId,
            selectedHingeEdgeId: hingeEdgeId,
            appliedAngles: renderedTreeAngles,
            collisionThickness,
            visualThickness: previewThickness,
          })
          if (!context) return false
          const ownerPlan = transitionFoldPreviewTreeMotionOwner(ownerState, {
            kind: 'prepare_runner',
            ownerToken: ownerState.ownerToken,
            generation: ownerState.generation,
            contextKey: context.contextKey,
            hingeEdgeId,
          })
          if (!ownerPlan?.accepted) return false
          treeMotionOwnerState = ownerPlan.state
          const prepareCommands = ownerPlan.commands.filter(
            (command) => command.kind === 'prepare_runner',
          )
          if (
            ownerPlan.commands.length !== 2
            || prepareCommands.length !== 1
            || ownerPlan.commands[0]?.kind !== 'dispose_runner'
            || ownerPlan.commands[1]?.kind !== 'prepare_runner'
          ) return false
          if (!executeTreeOwnershipCleanupCommand(ownerPlan.commands[0])) {
            return false
          }
          const prepareCommand = prepareCommands[0]
          if (
            !prepareCommand
            || prepareCommand.ownerToken !== ownerPlan.state.ownerToken
            || prepareCommand.generation !== ownerPlan.state.generation
            || prepareCommand.contextKey !== context.contextKey
            || prepareCommand.hingeEdgeId !== hingeEdgeId
          ) return false
          const motionRuntime = createFoldPreviewTreeMotionRuntime({
            context,
            ownerState: ownerPlan.state,
          })
          if (!motionRuntime) return false
          treeMotionRuntimeState = motionRuntime
          return true
        }
        const createTreeDirectPoseFrameTask = () =>
          createLatestFrameTask<PendingTreeDirectPose>(
            {
              request: (callback) => window.requestAnimationFrame(callback),
              cancel: (handle) => window.cancelAnimationFrame(handle),
            },
            (pendingPose) => {
              if (disposed) return
              const ownerState = treeMotionOwnerState
              if (!ownerState) return
              const ownerPlan = transitionFoldPreviewTreeMotionOwner(
                ownerState,
                {
                  kind: 'direct_callback',
                  ownerToken: pendingPose.ownerToken,
                  generation: pendingPose.generation,
                  key: pendingPose.requestKey,
                },
              )
              if (!ownerPlan?.accepted) return
              treeMotionOwnerState = ownerPlan.state
              const latestRequest = latestRequestedTreePoseRef.current
              if (
                !latestRequest
                || latestRequest.model !== model
                || latestRequest.fixedFaceId !== treeKinematics.rootFaceId
                || latestRequest.collisionThickness !== collisionThickness
                || latestRequest.visualThickness !== previewThickness
                || latestRequest.requestKey !== pendingPose.requestKey
              ) return
              const applyCommands = ownerPlan.commands.filter(
                (command) => command.kind === 'apply_direct',
              )
              const applyCommand = applyCommands[0]
              if (
                ownerPlan.commands.length !== 1
                || applyCommands.length !== 1
                || !applyCommand
                || applyCommand.ownerToken !== pendingPose.ownerToken
                || applyCommand.generation !== pendingPose.generation
                || applyCommand.key !== pendingPose.requestKey
              ) throw new Error('invalid tree direct pose command')
              if (
                !updatePose(pendingPose.angle, pendingPose.hingeAngles)
                || renderedTreePoseKey !== pendingPose.requestKey
              ) throw new Error('invalid fold tree pose')
              if (!prepareTreeMotionRuntime(selectedTreeHingeId)) {
                throw new Error('tree motion runtime is unavailable')
              }
              render()
            },
            () => {
              if (disposed) return
              dispose()
              setRenderError('3D描画を安全に継続できませんでした')
            },
          )
        const ensureTreeDirectPoseFrameTask = () => {
          if (!treeDirectPoseFrameTask) {
            treeDirectPoseFrameTask = createTreeDirectPoseFrameTask()
          }
          return treeDirectPoseFrameTask
        }

        if (
          renderedTreeAngles === null
          || renderedTreePoseKey === null
          || !createIdleTreeOwner()
          || !prepareTreeMotionRuntime(selectedTreeHingeId)
        ) throw new Error('tree motion runtime initialization failed')

        const updateTreeSelectionVisual = updateSelection
        updateSelection = (nextSelectedHingeId) => {
          selectedTreeHingeId = nextSelectedHingeId
          updateTreeSelectionVisual(nextSelectedHingeId)
          if (!prepareTreeMotionRuntime(nextSelectedHingeId)) {
            throw new Error('tree motion runtime selection failed')
          }
        }
        schedulePose = (
          nextAngle: number,
          nextHingeAngles?: readonly FoldPreviewHingeAngle[],
        ) => {
          const requestedPose = requestedTreePose(
            nextAngle,
            nextHingeAngles,
          )
          if (!requestedPose) return false
          if (
            requestedPose.requestKey === renderedTreePoseKey
            && !treeDirectPoseFrameTask?.hasPending()
            && treeMotionRuntimeState?.activeRequestSequence === null
          ) {
            resetTreeOwnedGesture()
            return prepareTreeMotionRuntime(selectedTreeHingeId)
          }
          if (!treeMotionOwnerState && !createIdleTreeOwner()) return false
          const ownerState = treeMotionOwnerState
          if (!ownerState) return false
          const ownerPlan = transitionFoldPreviewTreeMotionOwner(ownerState, {
            kind: 'external_direct_change',
            key: requestedPose.requestKey,
          })
          if (!ownerPlan?.accepted) return false
          treeMotionOwnerState = ownerPlan.state
          if (
            ownerPlan.commands.length !== 3
            || ownerPlan.commands[0]?.kind !== 'reset_gesture'
            || ownerPlan.commands[1]?.kind !== 'dispose_runner'
            || ownerPlan.commands[2]?.kind !== 'schedule_direct'
          ) return false
          if (
            !executeTreeOwnershipCleanupCommand(ownerPlan.commands[0])
            || !executeTreeOwnershipCleanupCommand(ownerPlan.commands[1])
          ) return false
          const scheduleCommand = ownerPlan.commands[2]
          if (
            scheduleCommand.ownerToken !== ownerPlan.state.ownerToken
            || scheduleCommand.generation !== ownerPlan.state.generation
            || scheduleCommand.key !== requestedPose.requestKey
          ) return false
          return ensureTreeDirectPoseFrameTask().schedule({
            ...requestedPose,
            ownerToken: scheduleCommand.ownerToken,
            generation: scheduleCommand.generation,
          })
        }
      } else {
        const requestedPoseKey = (
          requestedAngle: number,
          requestedHingeAngles?: readonly FoldPreviewHingeAngle[],
        ) => collisionPoseKey(
            model,
            resolvedFixedFaceId,
            collisionThickness,
            requestedAngle,
            requestedHingeAngles,
          )
        let appliedPoseKey = requestedPoseKey(
          initialPoseAngle,
          hingeAnglesRef.current,
        )
        if (!appliedPoseKey) {
          throw new Error('missing initial pose key')
        }
        const createdPoseFrameTask = createLatestFrameTask<PendingPose>(
          {
            request: (callback) => window.requestAnimationFrame(callback),
            cancel: (handle) => window.cancelAnimationFrame(handle),
          },
          (pendingPose) => {
            if (disposed) return
            if (!updatePose(pendingPose.angle, pendingPose.hingeAngles)) {
              throw new Error('invalid fold pose')
            }
            appliedPoseKey = pendingPose.requestKey
            render()
          },
          () => {
            if (disposed) return
            dispose()
            setRenderError('3D描画を安全に継続できませんでした')
          },
        )
        poseFrameTask = createdPoseFrameTask
        schedulePose = (
          nextAngle: number,
          nextHingeAngles?: readonly FoldPreviewHingeAngle[],
        ) => {
          const requestKey = requestedPoseKey(
            nextAngle,
            nextHingeAngles,
          )
          if (!requestKey) return false
          if (requestKey === appliedPoseKey && !createdPoseFrameTask.hasPending()) return true
          return createdPoseFrameTask.schedule({
            angle: nextAngle,
            hingeAngles: nextHingeAngles?.map((hingeAngle) => ({ ...hingeAngle })),
            requestKey,
          })
        }
      }
      const raycaster = new THREE.Raycaster()
      const pointer = new THREE.Vector2()
      const setPointerFromClient = (clientX: number, clientY: number) => {
        const bounds = createdRenderer.domElement.getBoundingClientRect()
        if (
          !isPositiveFinite(bounds.width)
          || !isPositiveFinite(bounds.height)
          || !Number.isFinite(clientX)
          || !Number.isFinite(clientY)
          || clientX < bounds.left
          || clientX > bounds.right
          || clientY < bounds.top
          || clientY > bounds.bottom
        ) return false
        pointer.set(
          ((clientX - bounds.left) / bounds.width) * 2 - 1,
          -((clientY - bounds.top) / bounds.height) * 2 + 1,
        )
        return true
      }
      const pickAt = (
        clientX: number,
        clientY: number,
      ): FoldPreviewPickTarget | null => {
        try {
          if (!setPointerFromClient(clientX, clientY)) return null
          scene.updateMatrixWorld(true)
          return pickFoldPreviewTarget(
            raycaster,
            camera,
            pointer,
            hingePickObjects,
            facePickObjects,
          )
        } catch {
          // Picking is optional; keep the verified render state unchanged.
          return null
        }
      }
      const pickSurfaceAt = (
        clientX: number,
        clientY: number,
        preferredFaceId?: string,
      ) => {
        try {
          if (!setPointerFromClient(clientX, clientY)) return null
          scene.updateMatrixWorld(true)
          return pickFoldPreviewFaceSurface(
            raycaster,
            camera,
            pointer,
            facePickObjects,
            preferredFaceId,
          )
        } catch {
          return null
        }
      }
      const pointerRayAt = (
        clientX: number,
        clientY: number,
      ): FoldPreviewPhysicalGrabRay | null => {
        try {
          if (!setPointerFromClient(clientX, clientY)) return null
          camera.updateMatrixWorld(true)
          raycaster.setFromCamera(pointer, camera)
          const { origin, direction } = raycaster.ray
          if (
            ![origin.x, origin.y, origin.z, direction.x, direction.y, direction.z]
              .every(Number.isFinite)
          ) return null
          const directionLength = direction.length()
          if (
            !Number.isFinite(directionLength)
            || Math.abs(directionLength - 1) > 1e-10
          ) return null
          return Object.freeze({
            origin: Object.freeze({ x: origin.x, y: origin.y, z: origin.z }),
            direction: Object.freeze({
              x: direction.x,
              y: direction.y,
              z: direction.z,
            }),
            minimumDistance: raycaster.near,
            maximumDistance: raycaster.far,
          })
        } catch {
          return null
        }
      }
      const selectAt = (clientX: number, clientY: number) => {
        const target = pickAt(clientX, clientY)
        if (target?.kind === 'hinge') {
          onSelectHingeRef.current?.(
            target.edgeId === selectedHingeIdRef.current ? null : target.edgeId,
          )
        } else if (target?.kind === 'face') {
          onChooseFixedFaceRef.current?.(target.faceId)
        } else {
          onSelectHingeRef.current?.(null)
        }
      }
      const canvas = createdRenderer.domElement
      const pointerDocument = canvas.ownerDocument
      const selectionGesture = createFoldPreviewSelectionGesture()

      const discardPendingAngleDragTarget = () => {
        if (angleDragFrameHandle !== null) {
          window.cancelAnimationFrame(angleDragFrameHandle)
          angleDragFrameHandle = null
        }
        hasPendingAngleDragTarget = false
      }

      const queueAngleDragTargetPresentation = () => {
        hasPendingAngleDragTarget = true
        if (angleDragFrameHandle !== null) return
        angleDragFrameHandle = window.requestAnimationFrame(() => {
          angleDragFrameHandle = null
          const hadPendingTarget = hasPendingAngleDragTarget
          hasPendingAngleDragTarget = false
          if (
            disposed
            || !hadPendingTarget
            || (
              angleDragState.kind !== 'dragging'
              && physicalGrabState.kind !== 'dragging'
            )
          ) return
          syncAngleDragPresentation()
        })
      }

      const releaseAngleDragCapture = (pointerId: number) => {
        if (angleDragCapturedPointerId !== pointerId) return
        if (canvas.hasPointerCapture(pointerId)) {
          attemptCleanup(() => canvas.releasePointerCapture(pointerId))
        }
        angleDragCapturedPointerId = null
        angleDragCapturedPointerType = null
      }

      const restoreCameraAfterAngleDrag = () => {
        if (
          !isCleanAngleDragState(angleDragState)
          || !isCleanPhysicalGrabState(physicalGrabState)
        ) return
        if (controlsEnabledBeforeAngleDrag !== null) {
          createdControls.enabled = controlsEnabledBeforeAngleDrag
          controlsEnabledBeforeAngleDrag = null
        }
        if (cursorBeforeAngleDrag !== null) {
          canvas.style.cursor = cursorBeforeAngleDrag
          cursorBeforeAngleDrag = null
        }
      }

      const syncAngleDragPresentation = () => {
        const sequence = angleDragSequenceRef.current
        if (physicalGrabState.kind === 'armed') {
          const state = physicalGrabState
          setAngleDragPresentation((current) => {
            const next: AngleDragPresentation = {
              state: 'armed',
              mapping: FOLD_PREVIEW_PHYSICAL_GRAB_MAPPING,
              pointerType: state.pointerType,
              captured: angleDragCapturedPointerId === state.pointerId,
              startApplied: state.session.appliedAngleDegrees,
              target: null,
              hingeId: angleDragHingeId,
              sequence,
              cameraControlsEnabled: createdControls.enabled,
            }
            return angleDragPresentationsEqual(current, next) ? current : next
          })
          return
        }
        if (physicalGrabState.kind === 'dragging') {
          const state = physicalGrabState
          setAngleDragPresentation((current) => {
            const next: AngleDragPresentation = {
              state: 'dragging',
              mapping: FOLD_PREVIEW_PHYSICAL_GRAB_MAPPING,
              pointerType: state.pointerType,
              captured: angleDragCapturedPointerId === state.pointerId,
              startApplied: state.session.appliedAngleDegrees,
              target: state.presentationTarget?.angleDegrees ?? null,
              hingeId: angleDragHingeId,
              sequence,
              cameraControlsEnabled: createdControls.enabled,
            }
            return angleDragPresentationsEqual(current, next) ? current : next
          })
          return
        }
        if (angleDragState.kind === 'armed') {
          const state = angleDragState
          setAngleDragPresentation((current) => {
            const next: AngleDragPresentation = {
              state: 'armed',
              mapping: FOLD_PREVIEW_ANGLE_DRAG_MAPPING,
              pointerType: state.pointerType,
              captured: angleDragCapturedPointerId === state.pointerId,
              startApplied: state.startAppliedAngle,
              target: null,
              hingeId: angleDragHingeId,
              sequence,
              cameraControlsEnabled: createdControls.enabled,
            }
            return angleDragPresentationsEqual(current, next) ? current : next
          })
          return
        }
        if (angleDragState.kind === 'dragging') {
          const state = angleDragState
          setAngleDragPresentation((current) => {
            const next: AngleDragPresentation = {
              state: 'dragging',
              mapping: FOLD_PREVIEW_ANGLE_DRAG_MAPPING,
              pointerType: state.pointerType,
              captured: angleDragCapturedPointerId === state.pointerId,
              startApplied: state.startAppliedAngle,
              target: current.state === 'dragging'
                && current.sequence === sequence
                ? current.target
                : state.targetAngle,
              hingeId: angleDragHingeId,
              sequence,
              cameraControlsEnabled: createdControls.enabled,
            }
            return angleDragPresentationsEqual(current, next) ? current : next
          })
          return
        }
        setAngleDragPresentation((current) => {
          const next: AngleDragPresentation = {
            ...INITIAL_ANGLE_DRAG_PRESENTATION,
            sequence,
            cameraControlsEnabled: createdControls.enabled,
          }
          return angleDragPresentationsEqual(current, next) ? current : next
        })
      }

      const submitAngleDragTarget = (targetAngle: number) => {
        if (
          disposed
          || model.kind !== 'single_fold'
          || angleDragHingeId !== model.hinge.edgeId
          || angleDragContextKey !== singleFoldMotionContextKey
          || singleFoldMotionContextKeyRef.current !== angleDragContextKey
          || !isFoldPreviewAngle(targetAngle)
          || continuousMotionRunner?.getState().status === 'disposed'
        ) return
        onRequestFoldAngleRef.current?.(targetAngle)
      }

      const applyAngleDragTransition = (
        transition: ReturnType<typeof reduceFoldPreviewAngleDrag>,
        eventPointerId: number | null,
      ) => {
        angleDragState = transition.state
        let handled = false
        for (const effect of transition.effects) {
          if (effect.kind === 'handled') {
            if (effect.pointerId === eventPointerId) handled = true
            continue
          }
          if (effect.kind === 'target') {
            queueAngleDragTargetPresentation()
            if (
              selectedHingeIdRef.current !== angleDragHingeId
              && angleDragHingeId
            ) {
              onSelectHingeRef.current?.(angleDragHingeId)
            }
            continue
          }
          if (effect.kind === 'cancel') {
            discardPendingAngleDragTarget()
            releaseAngleDragCapture(effect.pointerId)
            angleDragHingeId = null
            continue
          }
          discardPendingAngleDragTarget()
          const completedHingeId = angleDragHingeId
          releaseAngleDragCapture(effect.pointerId)
          if (effect.outcome === 'drag' && effect.targetAngle !== null) {
            submitAngleDragTarget(effect.targetAngle)
          } else if (completedHingeId) {
            onSelectHingeRef.current?.(
              completedHingeId === selectedHingeIdRef.current
                ? null
                : completedHingeId,
            )
          }
          angleDragHingeId = null
          angleDragContextKey = null
        }
        restoreCameraAfterAngleDrag()
        syncAngleDragPresentation()
        if (isCleanAngleDragState(angleDragState)) angleDragContextKey = null
        return handled
      }

      const resetAngleDrag = (
        reason: Extract<
          FoldPreviewAngleDragEvent,
          { kind: 'reset' }
        >['reason'],
      ) => {
        if (isCleanAngleDragState(angleDragState)) return false
        const transition = reduceFoldPreviewAngleDrag(angleDragState, {
          kind: 'reset',
          reason,
        })
        applyAngleDragTransition(transition, null)
        return true
      }

      const currentPhysicalGrabViewSnapshot = () => {
        const viewport = readFoldPreviewPhysicalGrabViewport(canvas)
        return viewport
          ? snapshotFoldPreviewPhysicalGrabView(
              camera,
              createdControls.target,
              viewport,
              angleRef.current,
            )
          : null
      }

      const currentPhysicalGrabGuardKey = () =>
        currentFoldPreviewPhysicalGrabGuardKey({
          guardKey: physicalGrabGuardKey,
          startedRunnerState: physicalGrabStartRunnerState,
          currentRunnerState: continuousMotionRunner?.getState() ?? null,
          startedViewKey: physicalGrabCameraSnapshot,
          currentViewKey: currentPhysicalGrabViewSnapshot(),
          activeContextKey: angleDragContextKey,
          renderedContextKey: singleFoldMotionContextKey,
          latestContextKey: singleFoldMotionContextKeyRef.current,
        })

      const physicalGrabEventGuardKey = () =>
        currentPhysicalGrabGuardKey()
        ?? 'stale-physical-grab-guard'

      const clearPhysicalGrabGuards = (clearEventSession: boolean) => {
        physicalGrabStartRunnerState = null
        physicalGrabCameraSnapshot = null
        physicalGrabGuardKey = null
        if (clearEventSession) physicalGrabSessionForEvents = null
      }

      const applyPhysicalGrabTransition = (
        transition: ReturnType<typeof reduceFoldPreviewPhysicalGrabGesture>,
        eventPointerId: number | null,
      ) => {
        const completionNeedsCurrentGuard = transition.effects.some(
          (effect) =>
            effect.kind === 'end'
            && effect.outcome === 'drag'
            && effect.completionTarget !== null,
        )
        const plan = planFoldPreviewPhysicalGrabTransition({
          transition,
          eventPointerId,
          selectedHingeId: selectedHingeIdRef.current,
          activeHingeId: angleDragHingeId,
          modelHingeId:
            model.kind === 'single_fold' ? model.hinge.edgeId : null,
          activeContextKey: angleDragContextKey,
          disposed,
          guardIsCurrent:
            completionNeedsCurrentGuard
            && currentPhysicalGrabGuardKey() !== null,
        })
        physicalGrabState = plan.state
        for (const command of plan.commands) {
          if (command.kind === 'queue_presentation') {
            queueAngleDragTargetPresentation()
            continue
          }
          if (command.kind === 'select_hinge') {
            attemptCleanup(() =>
              onSelectHingeRef.current?.(command.hingeId))
            continue
          }
          if (command.kind === 'discard_presentation') {
            discardPendingAngleDragTarget()
            continue
          }
          if (command.kind === 'release_capture') {
            releaseAngleDragCapture(command.pointerId)
            continue
          }
          if (command.kind === 'clear_interaction') {
            angleDragHingeId = null
            angleDragContextKey = null
            clearPhysicalGrabGuards(command.clearEventSession)
            continue
          }
          if (command.kind === 'restore_camera') {
            restoreCameraAfterAngleDrag()
            continue
          }
          if (command.kind === 'sync_presentation') {
            syncAngleDragPresentation()
            continue
          }
          attemptCleanup(() =>
            onRequestFoldAngleRef.current?.(command.angleDegrees))
        }
        return {
          handled: plan.handled,
          endedAsTap: plan.endedAsTap,
        }
      }

      const resetPhysicalGrab = (
        reason: Extract<
          FoldPreviewPhysicalGrabGestureEvent,
          { kind: 'reset' }
        >['reason'],
      ) => {
        if (isCleanPhysicalGrabState(physicalGrabState)) return false
        const transition = reduceFoldPreviewPhysicalGrabGesture(
          physicalGrabState,
          { kind: 'reset', reason },
        )
        applyPhysicalGrabTransition(transition, null)
        return true
      }

      const resetFoldGestures = (
        reason: 'reset' | 'window_blur' | 'dispose',
      ) => {
        const resetVertical = resetAngleDrag(reason)
        const resetPhysical = resetPhysicalGrab(reason)
        return resetVertical || resetPhysical
      }
      resetTreeOwnedGesture = () => {
        resetFoldGestures('reset')
      }

      const consumePointerEvent = (event: PointerEvent) => {
        if (event.cancelable) event.preventDefault()
        event.stopImmediatePropagation()
      }

      const angleDragContextIsCurrent = () =>
        angleDragContextKey !== null
        && angleDragContextKey === singleFoldMotionContextKey
        && angleDragContextKey === singleFoldMotionContextKeyRef.current

      const pointerWithinCanvas = (event: PointerEvent) => {
        const bounds = canvas.getBoundingClientRect()
        return isPositiveFinite(bounds.width)
          && isPositiveFinite(bounds.height)
          && Number.isFinite(event.clientX)
          && Number.isFinite(event.clientY)
          && event.clientX >= bounds.left
          && event.clientX <= bounds.right
          && event.clientY >= bounds.top
          && event.clientY <= bounds.bottom
      }

      pointerDownHandler = (event) => {
        try {
          const hadActivePointer = activeDocumentPointerIds.size > 0
          if (Number.isSafeInteger(event.pointerId) && event.pointerId >= 0) {
            activeDocumentPointerIds.add(event.pointerId)
          }
          if (
            isCleanAngleDragState(angleDragState)
            && isCleanPhysicalGrabState(physicalGrabState)
            && event.target !== canvas
          ) return
          if (
            !isCleanAngleDragState(angleDragState)
            && !angleDragContextIsCurrent()
          ) {
            resetAngleDrag('reset')
            consumePointerEvent(event)
            return
          }
          host.focus({ preventScroll: true })
          selectionGesture.pointerDown(pointerStart(event))
          const pointerType = angleDragPointerType(event)
          if (!isCleanPhysicalGrabState(physicalGrabState)) {
            const session = physicalGrabState.kind === 'armed'
              || physicalGrabState.kind === 'dragging'
              ? physicalGrabState.session
              : physicalGrabSessionForEvents
            if (!session) {
              selectionGesture.reset()
              resetPhysicalGrab('reset')
              consumePointerEvent(event)
              return
            }
            const transition = reduceFoldPreviewPhysicalGrabGesture(
              physicalGrabState,
              {
                kind: 'pointer_down',
                pointerId: event.pointerId,
                pointerType,
                clientX: event.clientX,
                clientY: event.clientY,
                button: event.button,
                buttons: event.buttons,
                isPrimary: event.isPrimary,
                altKey: event.altKey,
                ctrlKey: event.ctrlKey,
                metaKey: event.metaKey,
                shiftKey: event.shiftKey,
                hadActivePointer,
                guardKey: physicalGrabState.kind === 'armed'
                  || physicalGrabState.kind === 'dragging'
                  ? physicalGrabState.guardKey
                  : physicalGrabGuardKey ?? 'suppressed-physical-grab-guard',
                contextKey: physicalGrabState.kind === 'armed'
                  || physicalGrabState.kind === 'dragging'
                  ? physicalGrabState.contextKey
                  : session.contextKey,
                session,
              },
            )
            if (applyPhysicalGrabTransition(transition, event.pointerId).handled) {
              consumePointerEvent(event)
            }
            return
          }
          if (!isCleanAngleDragState(angleDragState)) {
            if (!pointerType) {
              resetAngleDrag('reset')
              consumePointerEvent(event)
              return
            }
            const bounds = canvas.getBoundingClientRect()
            const currentApplied =
              continuousMotionRunner?.getState().applied ?? 0
            const transition = reduceFoldPreviewAngleDrag(angleDragState, {
              kind: 'pointer_down',
              pointerId: event.pointerId,
              pointerType,
              clientX: event.clientX,
              clientY: event.clientY,
              button: event.button,
              isPrimary: event.isPrimary,
              altKey: event.altKey,
              ctrlKey: event.ctrlKey,
              metaKey: event.metaKey,
              shiftKey: event.shiftKey,
              hadActivePointer,
              appliedAngle: isFoldPreviewAngle(currentApplied) ? currentApplied : 0,
              viewportHeight: bounds.height,
            })
            if (applyAngleDragTransition(transition, event.pointerId)) {
              consumePointerEvent(event)
            }
            return
          }
          if (
            model.kind !== 'single_fold'
            || !singleAnchor
            || !onRequestFoldAngleRef.current
            || !continuousMotionRunner
            || !singleFoldMotionContextKey
            || !pointerType
          ) return
          const target = pickAt(event.clientX, event.clientY)
          const bounds = canvas.getBoundingClientRect()
          camera.updateMatrixWorld(true)
          if (
            target?.kind === 'hinge'
            && target.edgeId === model.hinge.edgeId
            && canBeginSingleFoldAngleDrag(
              camera,
              model.hinge,
              previewThickness / 2 + 0.008,
              bounds,
              event.clientX,
              event.clientY,
            )
          ) {
            const appliedAngle = continuousMotionRunner.getState().applied
            if (!isFoldPreviewAngle(appliedAngle)) return
            const transition = reduceFoldPreviewAngleDrag(angleDragState, {
              kind: 'pointer_down',
              pointerId: event.pointerId,
              pointerType,
              clientX: event.clientX,
              clientY: event.clientY,
              button: event.button,
              isPrimary: event.isPrimary,
              altKey: event.altKey,
              ctrlKey: event.ctrlKey,
              metaKey: event.metaKey,
              shiftKey: event.shiftKey,
              hadActivePointer,
              appliedAngle,
              viewportHeight: bounds.height,
            })
            if (transition.state.kind !== 'armed') return
            try {
              canvas.setPointerCapture(event.pointerId)
              if (!canvas.hasPointerCapture(event.pointerId)) {
                throw new Error('pointer capture was not acquired')
              }
            } catch {
              angleDragState = reduceFoldPreviewAngleDrag(transition.state, {
                kind: 'reset',
                reason: 'reset',
              }).state
              if (canvas.hasPointerCapture(event.pointerId)) {
                attemptCleanup(() => canvas.releasePointerCapture(event.pointerId))
              }
              return
            }
            angleDragCapturedPointerId = event.pointerId
            angleDragCapturedPointerType = pointerType
            angleDragHingeId = model.hinge.edgeId
            angleDragContextKey = singleFoldMotionContextKey
            controlsEnabledBeforeAngleDrag = createdControls.enabled
            cursorBeforeAngleDrag = canvas.style.cursor
            createdControls.enabled = false
            canvas.style.cursor = 'ns-resize'
            angleDragSequenceRef.current += 1
            if (applyAngleDragTransition(transition, event.pointerId)) {
              consumePointerEvent(event)
            }
            return
          }

          const runnerState = continuousMotionRunner.getState()
          if (
            runnerState.status === 'running'
            || runnerState.status === 'disposed'
            || !isFoldPreviewAngle(runnerState.applied)
          ) return
          const surfaceHit = pickSurfaceAt(
            event.clientX,
            event.clientY,
            singleAnchor.movingFace.id,
          )
          const startRay = pointerRayAt(event.clientX, event.clientY)
          const minimumOrbitRadius =
            model.worldUnitsPerMillimetre * 0.001
          if (
            !surfaceHit
            || !startRay
            || !isPositiveFinite(minimumOrbitRadius)
          ) return
          const prepared = prepareFoldPreviewSingleFoldPhysicalGrab({
            model,
            fixedFaceId: resolvedFixedFaceId ?? model.fixedFace.id,
            appliedAngleDegrees: runnerState.applied,
            contextKey: singleFoldMotionContextKey,
            surfaceHit,
            visualThickness: previewThickness,
            startRay,
            minimumOrbitRadius,
          })
          const physicalGrabViewport =
            readFoldPreviewPhysicalGrabViewport(canvas)
          if (
            prepared.kind !== 'ready'
            || !physicalGrabViewport
            || !canBeginFoldPreviewPhysicalGrabInView(
              camera,
              prepared.session,
              physicalGrabViewport,
            )
          ) return
          const cameraSnapshot = currentPhysicalGrabViewSnapshot()
          if (!cameraSnapshot) return
          const guardKey =
            `physical-grab-${physicalGrabGuardSequence + 1}`
          const transition = reduceFoldPreviewPhysicalGrabGesture(
            physicalGrabState,
            {
              kind: 'pointer_down',
              pointerId: event.pointerId,
              pointerType,
              clientX: event.clientX,
              clientY: event.clientY,
              button: event.button,
              buttons: event.buttons,
              isPrimary: event.isPrimary,
              altKey: event.altKey,
              ctrlKey: event.ctrlKey,
              metaKey: event.metaKey,
              shiftKey: event.shiftKey,
              hadActivePointer,
              guardKey,
              contextKey: singleFoldMotionContextKey,
              session: prepared.session,
            },
          )
          if (transition.state.kind !== 'armed') return
          try {
            canvas.setPointerCapture(event.pointerId)
            if (!canvas.hasPointerCapture(event.pointerId)) {
              throw new Error('pointer capture was not acquired')
            }
          } catch {
            physicalGrabState = reduceFoldPreviewPhysicalGrabGesture(
              transition.state,
              { kind: 'reset', reason: 'reset' },
            ).state
            if (canvas.hasPointerCapture(event.pointerId)) {
              attemptCleanup(() => canvas.releasePointerCapture(event.pointerId))
            }
            return
          }
          physicalGrabGuardSequence += 1
          physicalGrabStartRunnerState = runnerState
          physicalGrabCameraSnapshot = cameraSnapshot
          physicalGrabGuardKey = guardKey
          physicalGrabSessionForEvents = prepared.session
          angleDragCapturedPointerId = event.pointerId
          angleDragCapturedPointerType = pointerType
          angleDragHingeId = model.hinge.edgeId
          angleDragContextKey = singleFoldMotionContextKey
          controlsEnabledBeforeAngleDrag = createdControls.enabled
          cursorBeforeAngleDrag = canvas.style.cursor
          createdControls.enabled = false
          canvas.style.cursor = 'grabbing'
          angleDragSequenceRef.current += 1
          if (applyPhysicalGrabTransition(
            transition,
            event.pointerId,
          ).handled) {
            consumePointerEvent(event)
          }
        } catch {
          resetFoldGestures('reset')
          selectionGesture.reset()
        }
      }
      pointerMoveHandler = (event) => {
        selectionGesture.pointerMove(pointerSample(event))
        if (!isCleanPhysicalGrabState(physicalGrabState)) {
          if (physicalGrabState.kind === 'idle') {
            const transition = reduceFoldPreviewPhysicalGrabGesture(
              physicalGrabState,
              {
                kind: 'pointer_move',
                pointerId: event.pointerId,
                pointerType: angleDragPointerType(event),
                clientX: event.clientX,
                clientY: event.clientY,
                guardKey: 'suppressed-physical-grab-guard',
                contextKey:
                  physicalGrabSessionForEvents?.contextKey
                  ?? 'suppressed-physical-grab-context',
                ray: null,
                isInside: false,
                buttons: event.buttons,
              },
            )
            if (
              applyPhysicalGrabTransition(
                transition,
                event.pointerId,
              ).handled
            ) {
              consumePointerEvent(event)
            }
            return
          }
          let handled = false
          let samples: readonly PointerEvent[] = [event]
          try {
            const coalesced = event.getCoalescedEvents()
            const collected =
              collectFoldPreviewPhysicalGrabPointerSamples(
                event,
                coalesced,
              )
            if (!collected) {
              selectionGesture.reset()
              const transition = reduceFoldPreviewPhysicalGrabGesture(
                physicalGrabState,
                {
                  kind: 'pointer_move',
                  pointerId: event.pointerId,
                  pointerType: angleDragPointerType(event),
                  clientX: event.clientX,
                  clientY: event.clientY,
                  guardKey: physicalGrabEventGuardKey(),
                  contextKey:
                    singleFoldMotionContextKey
                    ?? physicalGrabSessionForEvents?.contextKey
                    ?? 'stale-physical-grab-context',
                  ray: null,
                  isInside: pointerWithinCanvas(event),
                  buttons: event.buttons,
                },
              )
              applyPhysicalGrabTransition(transition, event.pointerId)
              consumePointerEvent(event)
              return
            }
            samples = collected
          } catch {
            // The current event remains a complete pointer sample.
          }
          for (const sample of samples) {
            if (isCleanPhysicalGrabState(physicalGrabState)) break
            const pointerType = angleDragPointerType(sample)
            const ray = pointerRayAt(sample.clientX, sample.clientY)
            const transition = reduceFoldPreviewPhysicalGrabGesture(
              physicalGrabState,
              {
                kind: 'pointer_move',
                pointerId: sample.pointerId,
                pointerType,
                clientX: sample.clientX,
                clientY: sample.clientY,
                guardKey: physicalGrabEventGuardKey(),
                contextKey:
                  singleFoldMotionContextKey
                  ?? physicalGrabSessionForEvents?.contextKey
                  ?? 'stale-physical-grab-context',
                ray,
                isInside: pointerWithinCanvas(sample),
                buttons: sample.buttons,
              },
            )
            handled =
              applyPhysicalGrabTransition(transition, sample.pointerId).handled
              || handled
          }
          if (handled) consumePointerEvent(event)
          return
        }
        if (isCleanAngleDragState(angleDragState)) return
        if (!angleDragContextIsCurrent()) {
          selectionGesture.reset()
          resetAngleDrag('reset')
          consumePointerEvent(event)
          return
        }
        const pointerType = angleDragPointerType(event)
        if (
          !pointerType
          || (
            angleDragState.kind !== 'idle'
            && event.pointerId === angleDragState.pointerId
            && (
              (event.buttons & 1) === 0
              || !pointerWithinCanvas(event)
            )
          )
        ) {
          resetAngleDrag('reset')
          consumePointerEvent(event)
          return
        }
        const transition = reduceFoldPreviewAngleDrag(angleDragState, {
          kind: 'pointer_move',
          pointerId: event.pointerId,
          pointerType,
          clientX: event.clientX,
          clientY: event.clientY,
        })
        if (applyAngleDragTransition(transition, event.pointerId)) {
          consumePointerEvent(event)
        }
      }
      pointerUpHandler = (event) => {
        activeDocumentPointerIds.delete(event.pointerId)
        const selectionAccepted = selectionGesture.pointerUp(pointerSample(event))
        if (!isCleanPhysicalGrabState(physicalGrabState)) {
          const pointerType = angleDragPointerType(event)
          const ray = pointerRayAt(event.clientX, event.clientY)
          const transition = reduceFoldPreviewPhysicalGrabGesture(
            physicalGrabState,
            {
              kind: 'pointer_up',
              pointerId: event.pointerId,
              pointerType,
              clientX: event.clientX,
              clientY: event.clientY,
              guardKey: physicalGrabEventGuardKey(),
              contextKey:
                singleFoldMotionContextKey
                ?? physicalGrabSessionForEvents?.contextKey
                ?? 'stale-physical-grab-context',
              ray,
              isInside: pointerWithinCanvas(event),
              button: event.button,
              buttons: event.buttons,
            },
          )
          const result =
            applyPhysicalGrabTransition(transition, event.pointerId)
          if (result.endedAsTap && selectionAccepted) {
            selectAt(event.clientX, event.clientY)
          }
          if (result.handled) consumePointerEvent(event)
          return
        }
        if (isCleanAngleDragState(angleDragState)) {
          if (selectionAccepted) selectAt(event.clientX, event.clientY)
          return
        }
        if (!angleDragContextIsCurrent()) {
          selectionGesture.reset()
          resetAngleDrag('reset')
          consumePointerEvent(event)
          return
        }
        const pointerType = angleDragPointerType(event)
        if (
          !pointerType
          || (
            angleDragState.kind !== 'idle'
            && event.pointerId === angleDragState.pointerId
            && !pointerWithinCanvas(event)
          )
        ) {
          resetAngleDrag('reset')
          consumePointerEvent(event)
          return
        }
        const transition = reduceFoldPreviewAngleDrag(angleDragState, {
          kind: 'pointer_up',
          pointerId: event.pointerId,
          pointerType,
          clientX: event.clientX,
          clientY: event.clientY,
        })
        if (applyAngleDragTransition(transition, event.pointerId)) {
          consumePointerEvent(event)
        } else if (selectionAccepted) {
          selectAt(event.clientX, event.clientY)
        }
      }
      pointerCancelHandler = (event) => {
        activeDocumentPointerIds.delete(event.pointerId)
        selectionGesture.pointerCancel(event.pointerId)
        if (!isCleanPhysicalGrabState(physicalGrabState)) {
          const pointerType =
            angleDragPointerType(event) ?? angleDragCapturedPointerType
          const transition = reduceFoldPreviewPhysicalGrabGesture(
            physicalGrabState,
            {
              kind: 'pointer_cancel',
              pointerId: event.pointerId,
              pointerType,
              reason: 'pointer_cancel',
            },
          )
          if (
            applyPhysicalGrabTransition(
              transition,
              event.pointerId,
            ).handled
          ) {
            consumePointerEvent(event)
          }
          return
        }
        if (isCleanAngleDragState(angleDragState)) return
        const pointerType =
          angleDragPointerType(event) ?? angleDragCapturedPointerType
        if (!pointerType) {
          resetAngleDrag('reset')
          return
        }
        const transition = reduceFoldPreviewAngleDrag(angleDragState, {
          kind: 'pointer_cancel',
          pointerId: event.pointerId,
          pointerType,
          reason: 'pointer_cancel',
        })
        if (applyAngleDragTransition(transition, event.pointerId)) {
          consumePointerEvent(event)
        }
      }
      lostPointerCaptureHandler = (event) => {
        if (!isCleanPhysicalGrabState(physicalGrabState)) {
          const pointerType =
            angleDragPointerType(event) ?? angleDragCapturedPointerType
          const transition = reduceFoldPreviewPhysicalGrabGesture(
            physicalGrabState,
            {
              kind: 'pointer_cancel',
              pointerId: event.pointerId,
              pointerType,
              reason: 'lost_pointer_capture',
            },
          )
          applyPhysicalGrabTransition(transition, event.pointerId)
          return
        }
        if (isCleanAngleDragState(angleDragState)) return
        const pointerType =
          angleDragPointerType(event) ?? angleDragCapturedPointerType
        if (!pointerType) {
          resetAngleDrag('reset')
          return
        }
        const transition = reduceFoldPreviewAngleDrag(angleDragState, {
          kind: 'pointer_cancel',
          pointerId: event.pointerId,
          pointerType,
          reason: 'lost_pointer_capture',
        })
        applyAngleDragTransition(transition, event.pointerId)
      }
      windowBlurHandler = () => {
        activeDocumentPointerIds.clear()
        selectionGesture.reset()
        resetFoldGestures('window_blur')
      }
      pointerDocument.addEventListener('pointerdown', pointerDownHandler, true)
      pointerDocument.addEventListener('pointermove', pointerMoveHandler, true)
      pointerDocument.addEventListener('pointerup', pointerUpHandler, true)
      pointerDocument.addEventListener('pointercancel', pointerCancelHandler, true)
      canvas.addEventListener('lostpointercapture', lostPointerCaptureHandler)
      window.addEventListener('blur', windowBlurHandler)

      keyDownHandler = (event) => {
        if (event.target !== host) return
        if (
          !isCleanAngleDragState(angleDragState)
          || !isCleanPhysicalGrabState(physicalGrabState)
        ) {
          resetFoldGestures('reset')
          event.preventDefault()
          return
        }
        const command = resolveFoldPreviewCameraCommand(event)
        if (!command) return
        try {
          if (!applyFoldPreviewCameraCommand(
            createdControls,
            command,
            canvas.clientHeight,
          )) return
          event.preventDefault()
        } catch {
          dispose()
          setRenderError('3Dカメラ操作を安全に継続できませんでした')
        }
      }
      host.addEventListener('keydown', keyDownHandler)
      runtime = {
        schedulePose,
        updateSelection,
        render,
        cancelAngleDrag: () => {
          resetFoldGestures('reset')
        },
        resetView: () => createdControls.reset(),
        dispose,
      }
      runtimeRef.current = runtime

      const resize = () => {
        try {
          resetFoldGestures('reset')
          const size = readRenderableSize(host)
          if (!size) return
          camera.aspect = size.width / size.height
          camera.updateProjectionMatrix()
          createdRenderer.setSize(size.width, size.height, false)
          render()
        } catch {
          dispose()
          setRenderError('3D描画を安全に継続できませんでした')
        }
      }
      observer = typeof ResizeObserver === 'undefined'
        ? null
        : new ResizeObserver(resize)
      observer?.observe(host)
      render()
    } catch {
      dispose()
      setRenderError('このPCで3D描画を開始できませんでした')
      return
    }

    return dispose
  }, [
    model,
    previewThickness,
    collisionThickness,
    frontHex,
    frontOpacity,
    backHex,
    backOpacity,
    resolvedFixedFaceId,
    singleFoldMotionContextKey,
  ])

  useEffect(() => {
    const runtime = runtimeRef.current
    if (!runtime) return
    try {
      if (!runtime.schedulePose(safeAngle, hingeAngles)) {
        throw new Error('fold pose frame could not be scheduled')
      }
    } catch {
      runtime.dispose()
      setRenderError('3D描画を安全に継続できませんでした')
    }
  }, [
    safeAngle,
    hingeAngles,
    model,
    previewThickness,
    collisionThickness,
    frontHex,
    frontOpacity,
    backHex,
    backOpacity,
    resolvedFixedFaceId,
    singleFoldMotionContextKey,
  ])

  useEffect(() => {
    const runtime = runtimeRef.current
    if (!runtime) return
    try {
      runtime.updateSelection(selectedHingeId ?? null)
      runtime.render()
    } catch {
      runtime.dispose()
      setRenderError('3D選択表示を安全に継続できませんでした')
    }
  }, [selectedHingeId])

  const resetView = () => {
    const runtime = runtimeRef.current
    if (!runtime) return
    try {
      runtime.cancelAngleDrag()
      runtime.resetView()
    } catch {
      runtime.dispose()
      setRenderError('3Dカメラ操作を安全に継続できませんでした')
    }
  }

  const thicknessNote = !hasAuthoritativeThickness
    ? `紙厚入力が無効なため3D表示のみ ${formatMillimetres(safeThicknessMm)} mm（衝突判定には不使用）`
    : thicknessIsEmphasised
      ? `紙厚 ${formatMillimetres(safeThicknessMm)} mm（3D表示は視認用の最小厚、衝突判定は入力紙厚を使用）`
      : thicknessIsLimited
        ? `紙厚 ${formatMillimetres(safeThicknessMm)} mm（3D表示厚を上限調整、衝突判定は入力紙厚を使用）`
        : `紙厚 ${formatMillimetres(safeThicknessMm)} mm`
  const unavailableMessage = model && renderError
    ? renderError
    : statusMessage ?? '面・ヒンジ解析を待っています'
  const renderedTreePose =
    model?.kind === 'fold_graph'
    && model.kinematics.kind === 'tree'
    && renderedTreePoseSnapshot?.model === model
    && renderedTreePoseSnapshot.fixedFaceId
      === (resolvedFixedFaceId ?? model.kinematics.rootFaceId)
    && renderedTreePoseSnapshot.collisionThickness === collisionThickness
    && renderedTreePoseSnapshot.visualThickness === previewThickness
      ? renderedTreePoseSnapshot
      : null
  const treeAngleNote = renderedTreePose
    ? describeTreeAngles(renderedTreePose.appliedAngles, 0)
    : '姿勢を準備中'
  const fixedFaceIndex = model && resolvedFixedFaceId
    ? model.faces.findIndex((face) => face.id === resolvedFixedFaceId)
    : -1
  const fixedFaceLabel = fixedFaceIndex >= 0 ? `固定面 ${fixedFaceIndex + 1}` : null
  const fixedFaceNote = fixedFaceLabel ? `・${fixedFaceLabel}` : ''
  const contextualMotionState = model?.kind === 'single_fold'
    && singleFoldMotionContextKey
    && motionSnapshot?.contextKey === singleFoldMotionContextKey
    ? motionSnapshot.state
    : null
  const displayedAngle = model?.kind === 'single_fold'
    ? contextualMotionState?.applied ?? 0
    : safeAngle
  const currentMotionState = model?.kind === 'single_fold'
    ? contextualMotionState
      ? contextualMotionState.requested === safeAngle
        ? contextualMotionState
        : {
            ...contextualMotionState,
            requested: safeAngle,
            applied: displayedAngle,
            start: displayedAngle,
            status: 'running' as const,
            reason: null,
            result: null,
          }
      : null
    : null
  const motionFaceLabels: readonly FoldPreviewMotionFaceLabel[] =
    model?.kind === 'single_fold'
      ? model.faces.map((face, index) => ({
          id: face.id,
          number: index + 1,
          label: `面 ${index + 1}${face.id === resolvedFixedFaceId ? '（固定）' : ''}`,
        }))
      : []
  const motionView = model?.kind === 'single_fold' && !renderError
    ? describeFoldPreviewContinuousMotion(currentMotionState)
    : null
  const motionDetail = model?.kind === 'single_fold'
    && !renderError
    && angleDragPresentation.state === 'idle'
    ? describeFoldPreviewContinuousMotionDetail(currentMotionState, motionFaceLabels)
    : null
  const angleDragTarget = angleDragPresentation.state === 'dragging'
    ? angleDragPresentation.target
    : null
  const physicalGrabIsActive =
    angleDragPresentation.mapping === FOLD_PREVIEW_PHYSICAL_GRAB_MAPPING
  const angleDragActionLabel = physicalGrabIsActive
    ? '紙面ドラッグ'
    : '上下ドラッグ'
  const motionBadgeText = angleDragTarget !== null
    ? `${angleDragActionLabel}目標 ${formatAngle(angleDragTarget)}°・表示 ${formatAngle(displayedAngle)}° / 離すと検証`
    : angleDragPresentation.state === 'armed'
      ? `${angleDragActionLabel}待機・表示 ${formatAngle(displayedAngle)}°`
      : motionView?.badgeText
  const motionBadgeClass = angleDragPresentation.state === 'idle'
    ? motionView?.badgeClass
    : 'is-running'
  const currentCollisionRequestKey =
    model?.kind === 'fold_graph'
    && model.kinematics.kind === 'tree'
    ? renderedTreePose?.requestKey ?? ''
    : collisionPoseKey(
        model,
        resolvedFixedFaceId,
        collisionThickness,
        displayedAngle,
        model?.kind === 'single_fold' ? undefined : hingeAngles,
      )
  const currentCollisionSummary = collisionSummary?.requestKey === currentCollisionRequestKey
    ? collisionSummary
    : null
  const collisionPathDisclosure = model?.kind === 'single_fold'
    ? 'separately_reported'
    : 'unverified'
  const collisionNote = describeCollisionSummary(
    currentCollisionSummary,
    false,
    collisionPathDisclosure,
  )
  const previewPoseNote = model?.kind === 'fold_graph' && model.kinematics.kind === 'tree'
    ? `${model.faces.length}面・${model.hinges.length}ヒンジを${treeAngleNote}${fixedFaceNote}`
    : model?.kind === 'fold_graph'
      ? `${model.faces.length}面・${model.hinges.length}ヒンジは閉路拘束の平面確認段階`
      : model?.kind === 'single_fold' && fixedFaceLabel
        ? fixedFaceLabel
        : thicknessNote
  const basePreviewNote = previewPoseNote === thicknessNote
    ? `${previewPoseNote}・${collisionNote}`
    : `${previewPoseNote}・${collisionNote}・${thicknessNote}`
  const previewNote = model?.kind === 'single_fold'
    ? `${onRequestFoldAngle ? '移動面ドラッグで物理目標・折り目の上下ドラッグで角度指定・' : ''}${basePreviewNote}・ドラッグ中の姿勢は未変更・中央面・単一線形経路のみ`
    : basePreviewNote
  const collisionDescription = describeCollisionSummary(
    currentCollisionSummary,
    true,
    collisionPathDisclosure,
  )
  const previewImageDescription = model?.kind === 'single_fold' && !renderError
    ? `実展開図の3D折りプレビュー、表示角 ${displayedAngle}度、指定角 ${safeAngle}度${angleDragTarget === null ? '' : `、${angleDragActionLabel}中の未確認目標角 ${angleDragTarget}度。この目標角はポインターを離して経路検証が完了するまで3Dへ適用しません`}${fixedFaceNote}、${motionView?.accessibleText ?? ''}${motionDetail ? `。${motionDetail.summaryText}` : ''}、${collisionDescription}、${thicknessNote}`
    : model?.kind === 'fold_graph' && model.kinematics.kind === 'tree' && !renderError
      ? `実展開図の木構造複数面3D折りプレビュー、${model.faces.length}面・${model.hinges.length}ヒンジ、${treeAngleNote}${fixedFaceNote}、${collisionDescription}、${thicknessNote}`
      : model?.kind === 'fold_graph' && !renderError
        ? `実展開図の複数面3D平面確認、${model.faces.length}面・${model.hinges.length}ヒンジ、閉路拘束のため折り動作は未適用、${collisionDescription}、${thicknessNote}`
    : model?.kind === 'planar' && !renderError
      ? `実展開図の平面3Dプレビュー、${collisionDescription}、${thicknessNote}`
      : `3D折りプレビューは利用できません。${unavailableMessage}`
  const selectionDescription = onSelectHinge && onChooseFixedFace
    ? '。3D上のヒンジをクリックして選択し、面をクリックして固定面を変更できます'
    : onSelectHinge
      ? '。3D上のヒンジをクリックして選択できます'
      : onChooseFixedFace
        ? '。3D上の面をクリックして固定面を変更できます'
        : ''
  const angleDragDescription =
    model?.kind === 'single_fold' && onRequestFoldAngle
      ? '。3D上で移動する紙面の表または裏をつかんでドラッグすると、紙の回転軌道から折り角目標を作れます。折り目の上下ドラッグでは、上方向で増加、下方向で減少する角度パラメータ操作ができます。どちらの目標もドラッグ中は未確認で、ポインターを離して連続経路を確認した後にだけ3D表示へ適用されます。Altキーを押したドラッグはカメラ操作になります。キーボードでは下の指定折り量入力を使用できます'
      : ''
  const cameraDescription = model && !renderError
    ? `。マウスは${angleDragDescription ? '紙面と折り目の折り操作以外の場所を' : ''}左ドラッグで回転、ホイールまたは中ドラッグで拡大縮小、右ドラッグで平行移動できます。タッチは${angleDragDescription ? '紙面と折り目の折り操作以外を' : ''}1本指で回転、2本指で拡大縮小と平行移動ができます。キーボードは矢印キーで平行移動、Shiftと矢印キーで回転、プラスとマイナスで拡大縮小、Homeまたは0で視点をリセットできます`
    : ''
  const previewDescription =
    `${previewImageDescription}${selectionDescription}${angleDragDescription}${cameraDescription}`
  const previewAvailable = Boolean(model && !renderError)

  return (
    <div
      className="fold-preview"
      data-angle={displayedAngle}
      data-requested-angle={model?.kind === 'single_fold' ? safeAngle : undefined}
      data-applied-angle={model?.kind === 'single_fold' ? displayedAngle : undefined}
      data-motion-status={model?.kind === 'single_fold'
        ? previewAvailable ? motionView?.status : 'unavailable'
        : undefined}
      data-motion-runner-status={model?.kind === 'single_fold'
        ? currentMotionState?.status ?? (previewAvailable ? 'preparing' : 'unavailable')
        : undefined}
      data-motion-result-kind={
        motionDetail?.resultKind ?? currentMotionState?.result?.kind
      }
      data-motion-start-angle={currentMotionState?.start ?? undefined}
      data-motion-certified-safe-through={
        motionDetail?.certifiedSafeThrough
          ?? currentMotionState?.result?.certifiedSafeThrough
          ?? undefined
      }
      data-motion-bracket-start-time={motionDetail?.bracket?.progress[0]}
      data-motion-bracket-end-time={motionDetail?.bracket?.progress[1]}
      data-motion-bracket-start-angle={motionDetail?.bracket?.anglesInPathOrder[0]}
      data-motion-bracket-end-angle={motionDetail?.bracket?.anglesInPathOrder[1]}
      data-motion-reason={motionDetail?.reasonCode}
      data-motion-blocker-first-face-number={motionDetail?.firstFaceNumber ?? undefined}
      data-motion-blocker-second-face-number={motionDetail?.secondFaceNumber ?? undefined}
      data-motion-relation={motionDetail?.relation ?? undefined}
      data-motion-geometry-class={motionDetail?.geometryClass ?? undefined}
      data-motion-hinge-decision={motionDetail?.hingeDecision ?? undefined}
      data-angle-mode={hingeAngles ? 'per-hinge' : 'uniform'}
      data-angle-drag-mapping={
        model?.kind === 'single_fold' && onRequestFoldAngle
          ? FOLD_PREVIEW_ANGLE_DRAG_MAPPING
          : undefined
      }
      data-physical-grab-mapping={
        model?.kind === 'single_fold' && onRequestFoldAngle
          ? FOLD_PREVIEW_PHYSICAL_GRAB_MAPPING
          : undefined
      }
      data-active-angle-drag-mapping={
        angleDragPresentation.mapping ?? undefined
      }
      data-angle-drag-state={angleDragPresentation.state}
      data-angle-drag-pointer-type={angleDragPresentation.pointerType ?? undefined}
      data-angle-drag-captured={angleDragPresentation.captured}
      data-angle-drag-start-applied={angleDragPresentation.startApplied ?? undefined}
      data-angle-drag-target={angleDragPresentation.target ?? undefined}
      data-angle-drag-target-kind={
        angleDragTarget === null ? undefined : 'unverified_target'
      }
      data-angle-drag-hinge={angleDragPresentation.hingeId ?? undefined}
      data-angle-drag-sequence={angleDragPresentation.sequence}
      data-camera-controls-enabled={angleDragPresentation.cameraControlsEnabled}
      data-selected-hinge={selectedHingeId ?? undefined}
      data-fixed-face={resolvedFixedFaceId ?? undefined}
      data-interactive={Boolean(
        onSelectHinge || onChooseFixedFace || onRequestFoldAngle,
      )}
      data-topology-kind={model && !renderError ? model.kind : 'unavailable'}
      data-collision-thickness-world={collisionThickness ?? undefined}
      data-display-thickness-world={model ? previewThickness : undefined}
      data-collision-status={previewAvailable
        ? collisionDataStatus(currentCollisionSummary)
        : 'unavailable'}
      data-broad-phase-candidates={currentCollisionSummary?.kind === 'ready'
        ? currentCollisionSummary.totalCandidates
        : undefined}
      data-non-adjacent-candidates={currentCollisionSummary?.kind === 'ready'
        ? currentCollisionSummary.nonAdjacentCandidates
        : undefined}
      data-hinge-adjacent-candidates={currentCollisionSummary?.kind === 'ready'
        ? currentCollisionSummary.hingeAdjacentCandidates
        : undefined}
      data-narrow-interactions={currentCollisionSummary?.kind === 'ready'
        ? currentCollisionSummary.narrowInteractions
        : undefined}
      data-non-adjacent-penetrations={currentCollisionSummary?.kind === 'ready'
        ? currentCollisionSummary.nonAdjacentPenetrations
        : undefined}
      data-non-adjacent-contacts={currentCollisionSummary?.kind === 'ready'
        ? currentCollisionSummary.nonAdjacentContacts
        : undefined}
      data-hinge-interactions={currentCollisionSummary?.kind === 'ready'
        ? currentCollisionSummary.hingeInteractions
        : undefined}
      data-hinge-model-allowed-contacts={currentCollisionSummary?.kind === 'ready'
        ? currentCollisionSummary.hingeModelAllowedContacts
        : undefined}
      data-hinge-model-corridor-overlaps={currentCollisionSummary?.kind === 'ready'
        ? currentCollisionSummary.hingeModelCorridorOverlaps
        : undefined}
      data-hinge-outside-penetrations={currentCollisionSummary?.kind === 'ready'
        ? currentCollisionSummary.hingeOutsidePenetrations
        : undefined}
      data-hinge-outside-contacts={currentCollisionSummary?.kind === 'ready'
        ? currentCollisionSummary.hingeOutsideContacts
        : undefined}
      data-hinge-unresolved-interactions={currentCollisionSummary?.kind === 'ready'
        ? currentCollisionSummary.hingeUnresolvedInteractions
        : undefined}
      data-indeterminate-interactions={currentCollisionSummary?.kind === 'ready'
        ? currentCollisionSummary.indeterminateInteractions
        : undefined}
      role="group"
      aria-label="3D折りプレビュー"
    >
      <div
        ref={hostRef}
        className="fold-preview-viewport"
        role={previewAvailable ? 'region' : 'img'}
        aria-label={previewAvailable ? '3Dビュー' : previewDescription}
        aria-describedby={previewAvailable ? descriptionId : undefined}
        aria-keyshortcuts={previewAvailable
          ? 'ArrowUp ArrowDown ArrowLeft ArrowRight Shift+ArrowUp Shift+ArrowDown Shift+ArrowLeft Shift+ArrowRight + - Home 0'
          : undefined}
        tabIndex={previewAvailable ? 0 : -1}
      >
        {!model || renderError ? (
          <span className="fold-preview-empty" aria-hidden="true">{unavailableMessage}</span>
        ) : null}
        {previewAvailable ? (
          <div className="fold-preview-status-stack" aria-hidden="true">
            <span
              className={`fold-preview-collision ${collisionBadgeClass(currentCollisionSummary)}`}
              title={collisionDescription}
            >
              表示姿勢｜{collisionBadgeText(currentCollisionSummary)}
            </span>
            {motionView ? (
              <span
                className={`fold-preview-motion ${motionBadgeClass}`}
                title={angleDragPresentation.state === 'idle'
                  ? motionView.accessibleText
                  : motionBadgeText}
              >
                移動経路｜{motionBadgeText}
              </span>
            ) : null}
          </div>
        ) : null}
        {previewAvailable && motionDetail ? (
          <details
            className={`fold-preview-motion-detail is-${motionDetail.kind}`}
            open
          >
            <summary>{motionDetail.title}</summary>
            <dl>
              {motionDetail.rows.map((row, index) => (
                <div className={`is-${row.kind}`} key={`${row.label}-${index}`}>
                  <dt>{row.label}</dt>
                  <dd>{row.value}</dd>
                </div>
              ))}
            </dl>
          </details>
        ) : null}
        {previewAvailable ? (
          <span className="fold-preview-note" aria-hidden="true">{previewNote}</span>
        ) : null}
      </div>
      {previewAvailable ? (
        <span id={descriptionId} className="visually-hidden">{previewDescription}</span>
      ) : null}
      {previewAvailable && motionView ? (
        <span className="visually-hidden" aria-live="polite" aria-atomic="true">
          {motionView.terminalAnnouncement ?? ''}
        </span>
      ) : null}
      <button
        type="button"
        className="fold-preview-reset"
        disabled={!previewAvailable}
        onClick={resetView}
        title="カメラを初期位置へ戻す"
      >
        視点をリセット
      </button>
    </div>
  )
}

function pointerStart(event: PointerEvent) {
  return {
    pointerId: event.pointerId,
    clientX: event.clientX,
    clientY: event.clientY,
    button: event.button,
    isPrimary: event.isPrimary,
  }
}

function pointerSample(event: PointerEvent) {
  return {
    pointerId: event.pointerId,
    clientX: event.clientX,
    clientY: event.clientY,
  }
}

function angleDragPointerType(
  event: PointerEvent,
): FoldPreviewAngleDragPointerType | null {
  return event.pointerType === 'mouse'
    || event.pointerType === 'pen'
    || event.pointerType === 'touch'
    ? event.pointerType
    : null
}

function isCleanAngleDragState(state: FoldPreviewAngleDragState) {
  return state.kind === 'idle'
    && state.suppressedPointerIds.length === 0
    && !state.requiresReset
}

function isCleanPhysicalGrabState(
  state: FoldPreviewPhysicalGrabGestureState,
) {
  return state.kind === 'idle'
    && state.suppressedPointerIds.length === 0
    && !state.requiresReset
}

function canBeginSingleFoldAngleDrag(
  camera: THREE.Camera,
  hinge: Readonly<{
    start: Readonly<{ x: number; z: number }>
    end: Readonly<{ x: number; z: number }>
  }>,
  hingeY: number,
  bounds: Readonly<{
    left: number
    top: number
    width: number
    height: number
  }>,
  clientX: number,
  clientY: number,
) {
  if (
    !isPositiveFinite(bounds.width)
    || !isPositiveFinite(bounds.height)
    || !Number.isFinite(bounds.left)
    || !Number.isFinite(bounds.top)
    || !Number.isFinite(hingeY)
    || !Number.isFinite(clientX)
    || !Number.isFinite(clientY)
    || clientX < bounds.left
    || clientX > bounds.left + bounds.width
    || clientY < bounds.top
    || clientY > bounds.top + bounds.height
  ) return false
  const start = new THREE.Vector3(hinge.start.x, hingeY, hinge.start.z).project(camera)
  const end = new THREE.Vector3(hinge.end.x, hingeY, hinge.end.z).project(camera)
  return isFoldPreviewAngleDragScreenHit({
    viewport: bounds,
    pointer: { clientX, clientY },
    startNdc: start,
    endNdc: end,
    minimumLengthPixels: MIN_ANGLE_DRAG_HINGE_LENGTH_CSS,
    maximumDistancePixels: MAX_ANGLE_DRAG_HINGE_DISTANCE_CSS,
  })
}

function readFoldPreviewPhysicalGrabViewport(
  canvas: HTMLCanvasElement,
): FoldPreviewPhysicalGrabViewport | null {
  try {
    const bounds = canvas.getBoundingClientRect()
    if (
      !Number.isFinite(bounds.left)
      || !Number.isFinite(bounds.top)
      || !isPositiveFinite(bounds.width)
      || !isPositiveFinite(bounds.height)
      || !Number.isSafeInteger(canvas.clientWidth)
      || canvas.clientWidth <= 0
      || !Number.isSafeInteger(canvas.clientHeight)
      || canvas.clientHeight <= 0
    ) return null
    return Object.freeze({
      left: bounds.left,
      top: bounds.top,
      width: bounds.width,
      height: bounds.height,
      clientWidth: canvas.clientWidth,
      clientHeight: canvas.clientHeight,
    })
  } catch {
    return null
  }
}

function angleDragPresentationsEqual(
  first: AngleDragPresentation,
  second: AngleDragPresentation,
) {
  return first.state === second.state
    && first.mapping === second.mapping
    && first.pointerType === second.pointerType
    && first.captured === second.captured
    && first.startApplied === second.startApplied
    && first.target === second.target
    && first.hingeId === second.hingeId
    && first.sequence === second.sequence
    && first.cameraControlsEnabled === second.cameraControlsEnabled
}

function applyFoldRotation(
  pivot: THREE.Group,
  axis: THREE.Vector3,
  rotationSign: 1 | -1,
  angle: number,
) {
  pivot.quaternion.setFromAxisAngle(
    axis,
    THREE.MathUtils.degToRad(angle * rotationSign),
  )
}

function describeTreeAngles(
  hingeAngles: readonly FoldPreviewHingeAngle[] | undefined,
  uniformAngle: number,
) {
  if (!hingeAngles || hingeAngles.length === 0) return `一括 ${formatAngle(uniformAngle)}度`
  const values = hingeAngles.map(({ angleDegrees }) => angleDegrees)
  if (!values.every((value) => Number.isFinite(value) && value >= 0 && value <= 180)) {
    return '個別角度'
  }
  const minimum = Math.min(...values)
  const maximum = Math.max(...values)
  return minimum === maximum
    ? `全ヒンジ ${formatAngle(minimum)}度`
    : `個別 ${formatAngle(minimum)}〜${formatAngle(maximum)}度`
}

function formatAngle(value: number) {
  return value.toLocaleString('ja-JP', { maximumFractionDigits: 1 })
}

function resolveColor(color: RgbaColor | null | undefined, fallback: number) {
  if (!color) return { hex: fallback, opacity: 1 }
  const channels = [color.red, color.green, color.blue, color.alpha]
  if (!channels.every(Number.isFinite)) return { hex: fallback, opacity: 1 }
  const red = Math.round(THREE.MathUtils.clamp(color.red, 0, 255))
  const green = Math.round(THREE.MathUtils.clamp(color.green, 0, 255))
  const blue = Math.round(THREE.MathUtils.clamp(color.blue, 0, 255))
  const alpha = THREE.MathUtils.clamp(color.alpha, 0, 255) / 255
  return { hex: (red << 16) | (green << 8) | blue, opacity: alpha }
}

function createPaperMaterial(color: { hex: number; opacity: number }) {
  return new THREE.MeshStandardMaterial({
    color: color.hex,
    opacity: color.opacity,
    transparent: color.opacity < 1,
    roughness: 0.72,
  })
}

function mixColors(first: number, second: number) {
  const firstColor = new THREE.Color(first)
  const secondColor = new THREE.Color(second)
  return firstColor.lerp(secondColor, 0.5)
}

function attemptCleanup(action: () => void | undefined) {
  try {
    action()
  } catch {
    // Continue releasing the remaining independent WebGL resources.
  }
}

function disposeMaterial(material: THREE.Material | THREE.Material[]) {
  if (Array.isArray(material)) {
    new Set(material).forEach((item) => item.dispose())
    return
  }
  material.dispose()
}

function readRenderableSize(host: HTMLElement) {
  const width = host.clientWidth
  const height = host.clientHeight
  if (!isPositiveFinite(width) || !isPositiveFinite(height)) return null
  return { width, height }
}

function isPositiveFinite(value: number): value is number {
  return Number.isFinite(value) && value > 0
}

function isNonNegativeFinite(value: number | null | undefined): value is number {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0
}

function isFoldPreviewAngle(value: number) {
  return Number.isFinite(value) && value >= 0 && value <= 180
}

function formatMillimetres(value: number) {
  return value.toLocaleString('ja-JP', { maximumFractionDigits: 3 })
}
