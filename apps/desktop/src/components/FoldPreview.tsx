import { useEffect, useRef, useState } from 'react'
import * as THREE from 'three'
import type { RgbaColor } from '../lib/coreClient'
import { createFoldPreviewFaceGeometry } from '../lib/foldPreviewGeometry'
import type { FoldPreviewFaceModel, FoldPreviewModel } from '../lib/foldPreviewModel'

type FoldPreviewProps = {
  angle: number
  model?: FoldPreviewModel | null
  statusMessage?: string
  frontColor?: RgbaColor | null
  backColor?: RgbaColor | null
  thicknessMm?: number | null
}

type PreviewRuntime = {
  pivot: THREE.Group | null
  axis: THREE.Vector3 | null
  rotationSign: 1 | -1
  render: () => void
  dispose: () => void
}

const DEFAULT_THICKNESS_MM = 0.1
const MIN_VISIBLE_THICKNESS = 0.025
const MAX_VISIBLE_THICKNESS = 0.35

export function FoldPreview({
  angle,
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
    let fixedGeometry: THREE.BufferGeometry
    let movingGeometry: THREE.BufferGeometry | null = null
    try {
      fixedGeometry = createFoldPreviewFaceGeometry(
        model.fixedFace.polygon,
        previewThickness,
      )
      geometries.push(fixedGeometry)
      if (model.kind === 'single_fold') {
        const { start } = model.hinge
        movingGeometry = createFoldPreviewFaceGeometry(
          model.movingFace.polygon.map((point) => ({
            x: point.x - start.x,
            z: point.z - start.z,
          })),
          previewThickness,
        )
        geometries.push(movingGeometry)
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
    let hingeGeometry: THREE.BufferGeometry | null = null
    let hingeMaterial: THREE.LineBasicMaterial | null = null
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
      attemptCleanup(() => hingeGeometry?.dispose())
      if (grid) {
        attemptCleanup(() => grid?.geometry.dispose())
        attemptCleanup(() => disposeMaterial(grid?.material ?? []))
      }
      attemptCleanup(() => frontMaterial?.dispose())
      attemptCleanup(() => backMaterial?.dispose())
      attemptCleanup(() => sideMaterial?.dispose())
      attemptCleanup(() => edgeMaterial?.dispose())
      attemptCleanup(() => hingeMaterial?.dispose())
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

      scene.add(makeFace(fixedGeometry, model.fixedFace))

      let pivot: THREE.Group | null = null
      let axis: THREE.Vector3 | null = null
      let rotationSign: 1 | -1 = 1
      if (model.kind === 'single_fold' && movingGeometry) {
        pivot = new THREE.Group()
        pivot.position.set(model.hinge.start.x, 0, model.hinge.start.z)
        pivot.add(makeFace(movingGeometry, model.movingFace))
        axis = new THREE.Vector3(model.hinge.axis.x, 0, model.hinge.axis.z).normalize()
        rotationSign = model.hinge.rotationSign
        applyFoldRotation(pivot, axis, rotationSign, angleRef.current)
        scene.add(pivot)

        const createdHingeMaterial = new THREE.LineBasicMaterial({ color: 0x7a3f16 })
        hingeMaterial = createdHingeMaterial
        const createdHingeGeometry = new THREE.BufferGeometry()
        hingeGeometry = createdHingeGeometry
        createdHingeGeometry.setFromPoints([
          new THREE.Vector3(
            model.hinge.start.x,
            previewThickness / 2 + 0.008,
            model.hinge.start.z,
          ),
          new THREE.Vector3(
            model.hinge.end.x,
            previewThickness / 2 + 0.008,
            model.hinge.end.z,
          ),
        ])
        scene.add(new THREE.Line(createdHingeGeometry, createdHingeMaterial))
      }

      const render = () => createdRenderer.render(scene, camera)
      runtime = { pivot, axis, rotationSign, render, dispose }
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
      if (runtime.pivot && runtime.axis) {
        applyFoldRotation(runtime.pivot, runtime.axis, runtime.rotationSign, safeAngle)
      }
      runtime.render()
    } catch {
      runtime.dispose()
      setRenderError('3D描画を安全に継続できませんでした')
    }
  }, [safeAngle])

  const thicknessNote = thicknessIsEmphasised
    ? `紙厚 ${formatMillimetres(safeThicknessMm)} mm（3D表示は視認用の最小厚）`
    : thicknessIsLimited
      ? `紙厚 ${formatMillimetres(safeThicknessMm)} mm（3D表示厚を上限調整）`
      : `紙厚 ${formatMillimetres(safeThicknessMm)} mm`
  const unavailableMessage = model && renderError
    ? renderError
    : statusMessage ?? '面・ヒンジ解析を待っています'
  const previewDescription = model?.kind === 'single_fold' && !renderError
    ? `実展開図の3D折りプレビュー、折り角 ${safeAngle}度、${thicknessNote}`
    : model?.kind === 'planar' && !renderError
      ? `実展開図の平面3Dプレビュー、${thicknessNote}`
      : `3D折りプレビューは利用できません。${unavailableMessage}`

  return (
    <div
      ref={hostRef}
      className="fold-preview"
      data-angle={safeAngle}
      data-topology-kind={model && !renderError ? model.kind : 'unavailable'}
      role="img"
      aria-label={previewDescription}
    >
      {!model || renderError ? (
        <span className="fold-preview-empty">{unavailableMessage}</span>
      ) : null}
      {model && !renderError ? <span className="fold-preview-note">{thicknessNote}</span> : null}
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
