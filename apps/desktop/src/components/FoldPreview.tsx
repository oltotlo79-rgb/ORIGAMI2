import { useEffect, useRef, useState } from 'react'
import * as THREE from 'three'
import type { RgbaColor } from '../lib/coreClient'
import { createFoldPreviewFaceGeometry } from '../lib/foldPreviewGeometry'
import {
  calculateFoldTreePoseWithAngles,
  type FoldPreviewHingeAngle,
} from '../lib/foldPreviewKinematics'
import type { FoldPreviewFaceModel, FoldPreviewModel } from '../lib/foldPreviewModel'

type FoldPreviewProps = {
  angle: number
  hingeAngles?: readonly FoldPreviewHingeAngle[]
  selectedHingeId?: string | null
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

const DEFAULT_THICKNESS_MM = 0.1
const MIN_VISIBLE_THICKNESS = 0.025
const MAX_VISIBLE_THICKNESS = 0.35

export function FoldPreview({
  angle,
  hingeAngles,
  selectedHingeId,
  model,
  statusMessage,
  frontColor,
  backColor,
  thicknessMm,
}: FoldPreviewProps) {
  const hostRef = useRef<HTMLDivElement>(null)
  const runtimeRef = useRef<PreviewRuntime | null>(null)
  const [renderError, setRenderError] = useState<string | null>(null)
  // Assignment selects the fold direction; the control supplies only its magnitude.
  const safeAngle = Number.isFinite(angle) ? THREE.MathUtils.clamp(angle, 0, 180) : 0
  const angleRef = useRef(safeAngle)
  angleRef.current = safeAngle
  const hingeAnglesRef = useRef(hingeAngles)
  hingeAnglesRef.current = hingeAngles
  const selectedHingeIdRef = useRef(selectedHingeId ?? null)
  selectedHingeIdRef.current = selectedHingeId ?? null

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
      return
    }
    setRenderError(null)

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
        const fixedGeometry = createFoldPreviewFaceGeometry(
          model.fixedFace.polygon,
          previewThickness,
        )
        geometries.push(fixedGeometry)
        staticFaces.push({ face: model.fixedFace, geometry: fixedGeometry })

        const { start } = model.hinge
        movingGeometry = createFoldPreviewFaceGeometry(
          model.movingFace.polygon.map((point) => ({
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
    let hingeMaterial: THREE.LineBasicMaterial | null = null
    let selectedHingeMaterial: THREE.LineBasicMaterial | null = null
    let observer: ResizeObserver | null = null
    let runtime: PreviewRuntime | null = null
    let disposed = false

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

      const makeFace = (geometry: THREE.BufferGeometry, face: FoldPreviewFaceModel) => {
        const group = new THREE.Group()
        group.userData.faceId = face.id
        const paper = new THREE.Mesh(geometry, materials)
        paper.castShadow = true
        paper.receiveShadow = true
        const edgeGeometry = new THREE.EdgesGeometry(geometry, 20)
        edgeGeometries.push(edgeGeometry)
        group.add(paper, new THREE.LineSegments(edgeGeometry, createdEdgeMaterial))
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
        pivot = new THREE.Group()
        pivot.position.set(model.hinge.start.x, 0, model.hinge.start.z)
        pivot.add(makeFace(movingGeometry, model.movingFace))
        axis = new THREE.Vector3(model.hinge.axis.x, 0, model.hinge.axis.z).normalize()
        rotationSign = model.hinge.rotationSign
        applyFoldRotation(pivot, axis, rotationSign, angleRef.current)
        scene.add(pivot)
        updatePose = (nextAngle) => {
          if (!pivot || !axis) return false
          applyFoldRotation(pivot, axis, rotationSign, nextAngle)
          return true
        }
      }

      const hinges = model.kind === 'single_fold'
        ? [model.hinge]
        : model.kind === 'fold_graph'
          ? model.hinges
          : []
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
        }
        updateSelection(selectedHingeIdRef.current)

        if (model.kind === 'fold_graph' && model.kinematics.kind === 'tree') {
          const treeKinematics = model.kinematics
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
            return true
          }
          if (!updatePose(angleRef.current, hingeAnglesRef.current)) {
            throw new Error('invalid fold tree pose')
          }
        }
      }

      const render = () => createdRenderer.render(scene, camera)
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
    frontHex,
    frontOpacity,
    backHex,
    backOpacity,
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
  const previewNote = model?.kind === 'fold_graph' && model.kinematics.kind === 'tree'
    ? `${model.faces.length}面・${model.hinges.length}ヒンジを${treeAngleNote}（衝突未検証）・${thicknessNote}`
    : model?.kind === 'fold_graph'
      ? `${model.faces.length}面・${model.hinges.length}ヒンジは閉路拘束の平面確認段階・${thicknessNote}`
      : thicknessNote
  const previewDescription = model?.kind === 'single_fold' && !renderError
    ? `実展開図の3D折りプレビュー、折り角 ${safeAngle}度、${thicknessNote}`
    : model?.kind === 'fold_graph' && model.kinematics.kind === 'tree' && !renderError
      ? `実展開図の木構造複数面3D折りプレビュー、${model.faces.length}面・${model.hinges.length}ヒンジ、${treeAngleNote}、衝突未検証、${thicknessNote}`
      : model?.kind === 'fold_graph' && !renderError
        ? `実展開図の複数面3D平面確認、${model.faces.length}面・${model.hinges.length}ヒンジ、閉路拘束のため折り動作は未適用、${thicknessNote}`
    : model?.kind === 'planar' && !renderError
      ? `実展開図の平面3Dプレビュー、${thicknessNote}`
      : `3D折りプレビューは利用できません。${unavailableMessage}`

  return (
    <div
      ref={hostRef}
      className="fold-preview"
      data-angle={safeAngle}
      data-angle-mode={hingeAngles ? 'per-hinge' : 'uniform'}
      data-selected-hinge={selectedHingeId ?? undefined}
      data-topology-kind={model && !renderError ? model.kind : 'unavailable'}
      role="img"
      aria-label={previewDescription}
    >
      {!model || renderError ? (
        <span className="fold-preview-empty">{unavailableMessage}</span>
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
