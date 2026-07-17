import { useEffect, useRef, useState } from 'react'
import * as THREE from 'three'
import type { RgbaColor } from '../lib/coreClient'
import {
  collectFoldTreeDependentFaces,
  rerootFoldPreviewTree,
  resolveSingleFoldAnchor,
} from '../lib/foldPreviewAnchoring'
import {
  findFoldPreviewPoseBroadPhaseCandidates,
  type FoldPreviewCollisionAdjacency,
} from '../lib/foldPreviewCollision'
import { createFoldPreviewFaceGeometry } from '../lib/foldPreviewGeometry'
import {
  calculateFoldTreePoseWithAngles,
  type FoldPreviewHingeAngle,
} from '../lib/foldPreviewKinematics'
import type { FoldPreviewFaceModel, FoldPreviewModel } from '../lib/foldPreviewModel'
import {
  pickFoldPreviewTarget,
  type FoldPreviewPickObject,
} from '../lib/foldPreviewPicking'

type FoldPreviewProps = {
  angle: number
  hingeAngles?: readonly FoldPreviewHingeAngle[]
  selectedHingeId?: string | null
  fixedFaceId?: string | null
  onSelectHinge?: (edgeId: string | null) => void
  onChooseFixedFace?: (faceId: string) => void
  model?: FoldPreviewModel | null
  statusMessage?: string
  frontColor?: RgbaColor | null
  backColor?: RgbaColor | null
  thicknessMm?: number | null
}

type PreviewRuntime = {
  updatePose: (angle: number, hingeAngles?: readonly FoldPreviewHingeAngle[]) => boolean
  updateSelection: (selectedHingeId: string | null) => void
  render: () => void
  dispose: () => void
}

type CollisionSummary =
  | Readonly<{
      kind: 'ready'
      requestKey: string
      totalCandidates: number
      nonAdjacentCandidates: number
      hingeAdjacentCandidates: number
    }>
  | Readonly<{ kind: 'unavailable'; requestKey: string }>

const DEFAULT_THICKNESS_MM = 0.1
const MIN_VISIBLE_THICKNESS = 0.025
const MAX_VISIBLE_THICKNESS = 0.35

export function FoldPreview({
  angle,
  hingeAngles,
  selectedHingeId,
  fixedFaceId,
  onSelectHinge,
  onChooseFixedFace,
  model,
  statusMessage,
  frontColor,
  backColor,
  thicknessMm,
}: FoldPreviewProps) {
  const hostRef = useRef<HTMLDivElement>(null)
  const runtimeRef = useRef<PreviewRuntime | null>(null)
  const [renderError, setRenderError] = useState<string | null>(null)
  const [collisionSummary, setCollisionSummary] = useState<CollisionSummary | null>(null)
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
  const resolvedFixedFaceId = fixedFaceId
    ?? (model?.kind === 'single_fold'
      ? model.fixedFace.id
      : model?.kind === 'fold_graph' && model.kinematics.kind === 'tree'
        ? model.kinematics.rootFaceId
        : null)

  const safeThicknessMm = isNonNegativeFinite(thicknessMm) ? thicknessMm : DEFAULT_THICKNESS_MM
  const physicalPreviewThickness = model
    ? safeThicknessMm * model.worldUnitsPerMillimetre
    : 0
  const previewThickness = THREE.MathUtils.clamp(
    physicalPreviewThickness,
    MIN_VISIBLE_THICKNESS,
    MAX_VISIBLE_THICKNESS,
  )
  const { hex: frontHex, opacity: frontOpacity } = resolveColor(frontColor, 0xf5a65b)
  const { hex: backHex, opacity: backOpacity } = resolveColor(backColor, 0xfffdf9)
  const thicknessIsEmphasised = physicalPreviewThickness < MIN_VISIBLE_THICKNESS
  const thicknessIsLimited = physicalPreviewThickness > MAX_VISIBLE_THICKNESS

  useEffect(() => {
    const host = hostRef.current
    if (!host || !model) {
      runtimeRef.current = null
      setCollisionSummary(null)
      return
    }
    setRenderError(null)
    setCollisionSummary(null)

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
    let hingeMaterial: THREE.LineBasicMaterial | null = null
    let selectedHingeMaterial: THREE.LineBasicMaterial | null = null
    let observer: ResizeObserver | null = null
    let clickHandler: ((event: MouseEvent) => void) | null = null
    let runtime: PreviewRuntime | null = null
    let disposed = false
    const collisionAdjacencies: FoldPreviewCollisionAdjacency[] = model.kind === 'planar'
      ? []
      : (model.kind === 'single_fold' ? [model.hinge] : model.hinges).map((hinge) => ({
          edgeId: hinge.edgeId,
          firstFaceId: hinge.leftFaceId,
          secondFaceId: hinge.rightFaceId,
        }))

    const updateCollision = (
      faceTransforms: ReadonlyMap<string, THREE.Matrix4>,
      requestKey: string,
    ) => {
      let nextSummary: CollisionSummary = { kind: 'unavailable', requestKey }
      try {
        const result = findFoldPreviewPoseBroadPhaseCandidates(
          model.faces,
          faceTransforms,
          physicalPreviewThickness,
          collisionAdjacencies,
        )
        if (result) {
          const hingeAdjacentCandidates = result.candidates.reduce(
            (count, candidate) => count + Number(candidate.relation === 'hinge_adjacent'),
            0,
          )
          nextSummary = {
            kind: 'ready',
            requestKey,
            totalCandidates: result.candidates.length,
            nonAdjacentCandidates: result.candidates.length - hingeAdjacentCandidates,
            hingeAdjacentCandidates,
          }
        }
      } catch {
        // Broad-phase diagnostics are optional and must not invalidate a verified pose.
      }
      if (!disposed) {
        setCollisionSummary((current) =>
          collisionSummariesEqual(current, nextSummary) ? current : nextSummary)
      }
    }

    const dispose = () => {
      if (disposed) return
      disposed = true
      attemptCleanup(() => observer?.disconnect())
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
      attemptCleanup(() => hingeMaterial?.dispose())
      attemptCleanup(() => selectedHingeMaterial?.dispose())
      if (renderer) {
        if (clickHandler) {
          attemptCleanup(() => renderer?.domElement.removeEventListener('click', clickHandler!))
        }
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

      const faceEdgeLines = new Map<string, THREE.LineSegments>()
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
          faceGroups.set(face.id, group)
        }
        scene.add(group)
      }

      let pivot: THREE.Group | null = null
      let axis: THREE.Vector3 | null = null
      let rotationSign: 1 | -1 = 1
      let updatePose = (_angle: number, _hingeAngles?: readonly FoldPreviewHingeAngle[]) => true
      let updateSelection = (_selectedHingeId: string | null) => undefined
      if (model.kind === 'single_fold' && movingGeometry) {
        if (!singleAnchor) throw new Error('missing single-fold anchor')
        pivot = new THREE.Group()
        pivot.position.set(model.hinge.start.x, 0, model.hinge.start.z)
        pivot.add(makeFace(movingGeometry, singleAnchor.movingFace))
        axis = new THREE.Vector3(model.hinge.axis.x, 0, model.hinge.axis.z).normalize()
        rotationSign = singleAnchor.movingRotationSign
        scene.add(pivot)
        updatePose = (nextAngle) => {
          if (!pivot || !axis) return false
          applyFoldRotation(pivot, axis, rotationSign, nextAngle)
          updateCollision(new Map([
            [singleAnchor.fixedFace.id, new THREE.Matrix4()],
            [
              singleAnchor.movingFace.id,
              createFoldRotationTransform(model.hinge.start, axis, rotationSign, nextAngle),
            ],
          ]), collisionPoseKey(
            model,
            resolvedFixedFaceId,
            physicalPreviewThickness,
            nextAngle,
            undefined,
          ))
          return true
        }
        if (!updatePose(angleRef.current)) throw new Error('invalid single-fold pose')
      }

      const hinges = model.kind === 'single_fold'
        ? [model.hinge]
        : model.kind === 'fold_graph'
          ? model.hinges
          : []
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
          for (const [faceId, line] of faceEdgeLines) {
            const fixed = faceId === resolvedFixedFaceId
            const dependent = dependentFaceIds.has(faceId)
            line.material = fixed
              ? createdFixedFaceEdgeMaterial
              : dependent
                ? createdDependentFaceEdgeMaterial
                : createdEdgeMaterial
            line.renderOrder = fixed ? 9 : dependent ? 8 : 0
          }
        }
        updateSelection(selectedHingeIdRef.current)

        if (model.kind === 'fold_graph' && model.kinematics.kind === 'tree') {
          if (!treeKinematics) throw new Error('missing fold-tree anchor')
          updatePose = (nextAngle, nextHingeAngles) => {
            const pose = calculateFoldTreePoseWithAngles(treeKinematics, nextHingeAngles
              ? { kind: 'per_hinge', angles: nextHingeAngles }
              : { kind: 'uniform', angleDegrees: nextAngle })
            if (
              !pose
              || pose.faceTransforms.size !== faceGroups.size
              || pose.hingeTransforms.size !== hingeLines.size
            ) return false
            for (const [faceId, transform] of pose.faceTransforms) {
              const group = faceGroups.get(faceId)
              if (!group) return false
              group.matrix.copy(transform)
              group.matrixWorldNeedsUpdate = true
            }
            for (const [edgeId, transform] of pose.hingeTransforms) {
              const line = hingeLines.get(edgeId)
              if (!line) return false
              line.matrix.copy(transform)
              line.matrixWorldNeedsUpdate = true
            }
            updateCollision(pose.faceTransforms, collisionPoseKey(
              model,
              resolvedFixedFaceId,
              physicalPreviewThickness,
              nextAngle,
              nextHingeAngles,
            ))
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
        updateCollision(
          new Map(model.faces.map((face) => [face.id, new THREE.Matrix4()])),
          collisionPoseKey(
            model,
            resolvedFixedFaceId,
            physicalPreviewThickness,
            angleRef.current,
            hingeAnglesRef.current,
          ),
        )
      }

      const render = () => createdRenderer.render(scene, camera)
      const raycaster = new THREE.Raycaster()
      const pointer = new THREE.Vector2()
      clickHandler = (event) => {
        try {
          const bounds = createdRenderer.domElement.getBoundingClientRect()
          if (!isPositiveFinite(bounds.width) || !isPositiveFinite(bounds.height)) return
          pointer.set(
            ((event.clientX - bounds.left) / bounds.width) * 2 - 1,
            -((event.clientY - bounds.top) / bounds.height) * 2 + 1,
          )
          scene.updateMatrixWorld(true)
          const target = pickFoldPreviewTarget(
            raycaster,
            camera,
            pointer,
            hingePickObjects,
            facePickObjects,
          )
          if (target?.kind === 'hinge') {
            onSelectHingeRef.current?.(
              target.edgeId === selectedHingeIdRef.current ? null : target.edgeId,
            )
          } else if (target?.kind === 'face') {
            onChooseFixedFaceRef.current?.(target.faceId)
          } else {
            onSelectHingeRef.current?.(null)
          }
        } catch {
          // Picking is optional; keep the verified render state unchanged.
        }
      }
      createdRenderer.domElement.addEventListener('click', clickHandler)
      runtime = { updatePose, updateSelection, render, dispose }
      runtimeRef.current = runtime

      const resize = () => {
        try {
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
    physicalPreviewThickness,
    frontHex,
    frontOpacity,
    backHex,
    backOpacity,
    resolvedFixedFaceId,
  ])

  useEffect(() => {
    const runtime = runtimeRef.current
    if (!runtime) return
    try {
      if (!runtime.updatePose(safeAngle, hingeAngles)) throw new Error('invalid fold pose')
      runtime.render()
    } catch {
      runtime.dispose()
      setRenderError('3D描画を安全に継続できませんでした')
    }
  }, [safeAngle, hingeAngles])

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

  const thicknessNote = thicknessIsEmphasised
    ? `紙厚 ${formatMillimetres(safeThicknessMm)} mm（3D表示は視認用の最小厚）`
    : thicknessIsLimited
      ? `紙厚 ${formatMillimetres(safeThicknessMm)} mm（3D表示厚を上限調整）`
      : `紙厚 ${formatMillimetres(safeThicknessMm)} mm`
  const unavailableMessage = model && renderError
    ? renderError
    : statusMessage ?? '面・ヒンジ解析を待っています'
  const treeAngleNote = describeTreeAngles(hingeAngles, safeAngle)
  const fixedFaceIndex = model && resolvedFixedFaceId
    ? model.faces.findIndex((face) => face.id === resolvedFixedFaceId)
    : -1
  const fixedFaceLabel = fixedFaceIndex >= 0 ? `固定面 ${fixedFaceIndex + 1}` : null
  const fixedFaceNote = fixedFaceLabel ? `・${fixedFaceLabel}` : ''
  const currentCollisionRequestKey = collisionPoseKey(
    model,
    resolvedFixedFaceId,
    physicalPreviewThickness,
    safeAngle,
    hingeAngles,
  )
  const currentCollisionSummary = collisionSummary?.requestKey === currentCollisionRequestKey
    ? collisionSummary
    : null
  const collisionNote = describeCollisionSummary(currentCollisionSummary)
  const previewPoseNote = model?.kind === 'fold_graph' && model.kinematics.kind === 'tree'
    ? `${model.faces.length}面・${model.hinges.length}ヒンジを${treeAngleNote}${fixedFaceNote}`
    : model?.kind === 'fold_graph'
      ? `${model.faces.length}面・${model.hinges.length}ヒンジは閉路拘束の平面確認段階`
      : model?.kind === 'single_fold' && fixedFaceLabel
        ? fixedFaceLabel
        : thicknessNote
  const previewNote = previewPoseNote === thicknessNote
    ? `${previewPoseNote}・${collisionNote}`
    : `${previewPoseNote}・${collisionNote}・${thicknessNote}`
  const collisionDescription = describeCollisionSummary(currentCollisionSummary, true)
  const previewImageDescription = model?.kind === 'single_fold' && !renderError
    ? `実展開図の3D折りプレビュー、折り角 ${safeAngle}度${fixedFaceNote}、${collisionDescription}、${thicknessNote}`
    : model?.kind === 'fold_graph' && model.kinematics.kind === 'tree' && !renderError
      ? `実展開図の木構造複数面3D折りプレビュー、${model.faces.length}面・${model.hinges.length}ヒンジ、${treeAngleNote}${fixedFaceNote}、${collisionDescription}、${thicknessNote}`
      : model?.kind === 'fold_graph' && !renderError
        ? `実展開図の複数面3D平面確認、${model.faces.length}面・${model.hinges.length}ヒンジ、閉路拘束のため折り動作は未適用、${collisionDescription}、${thicknessNote}`
    : model?.kind === 'planar' && !renderError
      ? `実展開図の平面3Dプレビュー、${collisionDescription}、${thicknessNote}`
      : `3D折りプレビューは利用できません。${unavailableMessage}`
  const interactionDescription = onSelectHinge && onChooseFixedFace
    ? '。3D上のヒンジをクリックして選択し、面をクリックして固定面を変更できます'
    : onSelectHinge
      ? '。3D上のヒンジをクリックして選択できます'
      : onChooseFixedFace
        ? '。3D上の面をクリックして固定面を変更できます'
        : ''
  const previewDescription = `${previewImageDescription}${interactionDescription}`

  return (
    <div
      ref={hostRef}
      className="fold-preview"
      data-angle={safeAngle}
      data-angle-mode={hingeAngles ? 'per-hinge' : 'uniform'}
      data-selected-hinge={selectedHingeId ?? undefined}
      data-fixed-face={resolvedFixedFaceId ?? undefined}
      data-interactive={Boolean(onSelectHinge || onChooseFixedFace)}
      data-topology-kind={model && !renderError ? model.kind : 'unavailable'}
      data-collision-status={collisionDataStatus(currentCollisionSummary)}
      data-broad-phase-candidates={currentCollisionSummary?.kind === 'ready'
        ? currentCollisionSummary.totalCandidates
        : undefined}
      data-non-adjacent-candidates={currentCollisionSummary?.kind === 'ready'
        ? currentCollisionSummary.nonAdjacentCandidates
        : undefined}
      role="img"
      aria-label={previewDescription}
    >
      {!model || renderError ? (
        <span className="fold-preview-empty">{unavailableMessage}</span>
      ) : null}
      {model && !renderError ? (
        <span
          className={`fold-preview-collision ${collisionBadgeClass(currentCollisionSummary)}`}
          title={collisionDescription}
        >
          {collisionBadgeText(currentCollisionSummary)}
        </span>
      ) : null}
      {model && !renderError ? <span className="fold-preview-note">{previewNote}</span> : null}
    </div>
  )
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

function createFoldRotationTransform(
  start: Readonly<{ x: number; z: number }>,
  axis: THREE.Vector3,
  rotationSign: 1 | -1,
  angle: number,
) {
  return new THREE.Matrix4()
    .makeTranslation(start.x, 0, start.z)
    .multiply(new THREE.Matrix4().makeRotationAxis(
      axis,
      THREE.MathUtils.degToRad(angle * rotationSign),
    ))
    .multiply(new THREE.Matrix4().makeTranslation(-start.x, 0, -start.z))
}

function collisionSummariesEqual(
  first: CollisionSummary | null,
  second: CollisionSummary,
) {
  if (
    !first
    || first.kind !== second.kind
    || first.requestKey !== second.requestKey
  ) return false
  return first.kind === 'unavailable'
    || (
      second.kind === 'ready'
      && first.totalCandidates === second.totalCandidates
      && first.nonAdjacentCandidates === second.nonAdjacentCandidates
      && first.hingeAdjacentCandidates === second.hingeAdjacentCandidates
    )
}

function collisionPoseKey(
  model: FoldPreviewModel | null | undefined,
  fixedFaceId: string | null,
  thickness: number,
  angle: number,
  hingeAngles: readonly FoldPreviewHingeAngle[] | undefined,
) {
  if (!model) return ''
  const orderedHingeAngles = hingeAngles
    ? hingeAngles
      .map(({ edgeId, angleDegrees }) => [edgeId, angleDegrees] as const)
      .sort((first, second) => compareText(first[0], second[0]))
    : null
  return JSON.stringify([
    model.projectId,
    model.revision,
    model.kind,
    fixedFaceId,
    thickness,
    angle,
    orderedHingeAngles,
  ])
}

function compareText(first: string, second: string) {
  return first < second ? -1 : first > second ? 1 : 0
}

function describeCollisionSummary(summary: CollisionSummary | null, accessible = false) {
  if (!summary) return accessible ? '衝突候補の広域判定中' : '広域判定中'
  if (summary.kind === 'unavailable') {
    return accessible ? '衝突候補の広域判定は利用できません' : '広域判定不能'
  }
  if (summary.totalCandidates === 0) {
    return accessible
      ? '広域衝突候補は0件、狭域衝突判定は未実装'
      : '広域候補 0（狭域判定は未実装）'
  }
  return accessible
    ? `広域衝突候補は${summary.totalCandidates}件、非隣接${summary.nonAdjacentCandidates}件、ヒンジ隣接${summary.hingeAdjacentCandidates}件。衝突確定ではありません`
    : `広域候補 ${summary.totalCandidates}（非隣接 ${summary.nonAdjacentCandidates}・ヒンジ隣接 ${summary.hingeAdjacentCandidates}、衝突確定ではありません）`
}

function collisionDataStatus(summary: CollisionSummary | null) {
  return !summary ? 'pending' : summary.kind
}

function collisionBadgeClass(summary: CollisionSummary | null) {
  if (!summary || summary.kind === 'unavailable') return 'is-unavailable'
  if (summary.totalCandidates === 0) return 'is-clear'
  return summary.nonAdjacentCandidates > 0 ? 'has-other-candidates' : 'has-hinge-candidates'
}

function collisionBadgeText(summary: CollisionSummary | null) {
  if (!summary) return '広域判定中'
  if (summary.kind === 'unavailable') return '広域判定不能'
  return summary.totalCandidates === 0
    ? '広域候補 0'
    : `広域候補 ${summary.totalCandidates}・非隣接 ${summary.nonAdjacentCandidates}`
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

function formatMillimetres(value: number) {
  return value.toLocaleString('ja-JP', { maximumFractionDigits: 3 })
}
