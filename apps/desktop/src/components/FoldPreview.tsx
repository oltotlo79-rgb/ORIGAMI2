import { useEffect, useRef } from 'react'
import * as THREE from 'three'
import type { RgbaColor } from '../lib/coreClient'
import type { PaperBounds } from './CreaseCanvas'

type FoldPreviewProps = {
  angle: number
  paperBounds?: PaperBounds | null
  frontColor?: RgbaColor | null
  backColor?: RgbaColor | null
  thicknessMm?: number | null
}

type PreviewRuntime = {
  pivot: THREE.Group
  render: () => void
}

const DEFAULT_PAPER_SIZE_MM = 400
const DEFAULT_THICKNESS_MM = 0.1
const MAX_PAPER_WORLD_SIZE = 4.4
const MIN_VISIBLE_SIDE = 0.04
const MIN_VISIBLE_THICKNESS = 0.025
const MAX_VISIBLE_THICKNESS = 0.35

export function FoldPreview({
  angle,
  paperBounds,
  frontColor,
  backColor,
  thicknessMm,
}: FoldPreviewProps) {
  const hostRef = useRef<HTMLDivElement>(null)
  const runtimeRef = useRef<PreviewRuntime | null>(null)
  const safeAngle = Number.isFinite(angle) ? THREE.MathUtils.clamp(angle, -360, 360) : 0
  const angleRef = useRef(safeAngle)
  angleRef.current = safeAngle

  const paperSize = resolvePaperSize(paperBounds)
  const largestPaperDimension = Math.max(paperSize.width, paperSize.height)
  const previewWidth = Math.max(
    MAX_PAPER_WORLD_SIZE * (paperSize.width / largestPaperDimension),
    MIN_VISIBLE_SIDE,
  )
  const previewDepth = Math.max(
    MAX_PAPER_WORLD_SIZE * (paperSize.height / largestPaperDimension),
    MIN_VISIBLE_SIDE,
  )
  const safeThicknessMm = isNonNegativeFinite(thicknessMm) ? thicknessMm : DEFAULT_THICKNESS_MM
  const physicalPreviewThickness = safeThicknessMm === 0
    ? 0
    : MAX_PAPER_WORLD_SIZE * (safeThicknessMm / largestPaperDimension)
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
    if (!host) return

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

    const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false })
    const devicePixelRatio = Number.isFinite(window.devicePixelRatio) && window.devicePixelRatio > 0
      ? window.devicePixelRatio
      : 1
    renderer.setPixelRatio(Math.min(devicePixelRatio, 2))
    renderer.setSize(initialSize?.width ?? 1, initialSize?.height ?? 1, false)
    renderer.outputColorSpace = THREE.SRGBColorSpace
    renderer.shadowMap.enabled = true
    renderer.shadowMap.type = THREE.PCFSoftShadowMap
    host.appendChild(renderer.domElement)

    scene.add(new THREE.HemisphereLight(0xffffff, 0x748090, 2.2))
    const light = new THREE.DirectionalLight(0xffffff, 2.5)
    light.position.set(3, 7, 4)
    light.castShadow = true
    scene.add(light)

    const grid = new THREE.GridHelper(8, 16, 0xb8c1cc, 0xd7dde4)
    grid.position.y = -0.85
    scene.add(grid)

    const frontMaterial = createPaperMaterial({ hex: frontHex, opacity: frontOpacity })
    const backMaterial = createPaperMaterial({ hex: backHex, opacity: backOpacity })
    const sideMaterial = new THREE.MeshStandardMaterial({
      color: mixColors(frontHex, backHex),
      roughness: 0.82,
    })
    // BoxGeometry material order: +x, -x, +y, -y, +z, -z.
    // The paper lies in XZ, so +y is the front and -y is the back.
    const materials = [
      sideMaterial,
      sideMaterial,
      frontMaterial,
      backMaterial,
      sideMaterial,
      sideMaterial,
    ]
    const halfGeometry = new THREE.BoxGeometry(previewWidth / 2, previewThickness, previewDepth)
    const edgeGeometry = new THREE.EdgesGeometry(halfGeometry)
    const edgeMaterial = new THREE.LineBasicMaterial({ color: 0x715747 })

    const makeHalf = () => {
      const group = new THREE.Group()
      const paper = new THREE.Mesh(halfGeometry, materials)
      paper.castShadow = true
      paper.receiveShadow = true
      group.add(paper, new THREE.LineSegments(edgeGeometry, edgeMaterial))
      return group
    }

    const left = makeHalf()
    left.position.x = -previewWidth / 4
    const rightPivot = new THREE.Group()
    const right = makeHalf()
    right.position.x = previewWidth / 4
    rightPivot.add(right)
    rightPivot.rotation.z = THREE.MathUtils.degToRad(angleRef.current)
    scene.add(left, rightPivot)

    const hingeMaterial = new THREE.LineBasicMaterial({ color: 0x7a3f16 })
    const hingeGeometry = new THREE.BufferGeometry().setFromPoints([
      new THREE.Vector3(0, previewThickness / 2 + 0.008, -previewDepth / 2),
      new THREE.Vector3(0, previewThickness / 2 + 0.008, previewDepth / 2),
    ])
    scene.add(new THREE.Line(hingeGeometry, hingeMaterial))

    const render = () => renderer.render(scene, camera)
    runtimeRef.current = { pivot: rightPivot, render }

    const resize = () => {
      const size = readRenderableSize(host)
      if (!size) return
      camera.aspect = size.width / size.height
      camera.updateProjectionMatrix()
      renderer.setSize(size.width, size.height, false)
      render()
    }
    const observer = typeof ResizeObserver === 'undefined'
      ? null
      : new ResizeObserver(resize)
    observer?.observe(host)
    // Even a hidden/zero-sized host gets a safe 1x1 frame. A later resize will
    // replace it when ResizeObserver reports a renderable size.
    render()

    return () => {
      observer?.disconnect()
      if (runtimeRef.current?.pivot === rightPivot) runtimeRef.current = null
      halfGeometry.dispose()
      edgeGeometry.dispose()
      hingeGeometry.dispose()
      grid.geometry.dispose()
      disposeMaterial(grid.material)
      frontMaterial.dispose()
      backMaterial.dispose()
      sideMaterial.dispose()
      edgeMaterial.dispose()
      hingeMaterial.dispose()
      renderer.dispose()
      renderer.domElement.remove()
    }
  }, [
    previewWidth,
    previewDepth,
    previewThickness,
    frontHex,
    frontOpacity,
    backHex,
    backOpacity,
  ])

  useEffect(() => {
    const runtime = runtimeRef.current
    if (!runtime) return
    runtime.pivot.rotation.z = THREE.MathUtils.degToRad(safeAngle)
    runtime.render()
  }, [safeAngle])

  const thicknessNote = thicknessIsEmphasised
    ? `紙厚 ${formatMillimetres(safeThicknessMm)} mm（3D表示は視認用の最小厚）`
    : thicknessIsLimited
      ? `紙厚 ${formatMillimetres(safeThicknessMm)} mm（3D表示厚を上限調整）`
      : `紙厚 ${formatMillimetres(safeThicknessMm)} mm`

  return (
    <div
      ref={hostRef}
      className="fold-preview"
      data-angle={safeAngle}
      data-paper-aspect={`${paperSize.width}:${paperSize.height}`}
      role="img"
      aria-label={`用紙の3D折りプレビュー、折り角 ${safeAngle}度、${thicknessNote}`}
    >
      <span className="fold-preview-note">{thicknessNote}</span>
    </div>
  )
}

function resolvePaperSize(bounds?: PaperBounds | null) {
  if (!bounds) return { width: DEFAULT_PAPER_SIZE_MM, height: DEFAULT_PAPER_SIZE_MM }
  const width = bounds.maxX - bounds.minX
  const height = bounds.maxY - bounds.minY
  if (!isPositiveFinite(width) || !isPositiveFinite(height)) {
    return { width: DEFAULT_PAPER_SIZE_MM, height: DEFAULT_PAPER_SIZE_MM }
  }
  return { width, height }
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

function disposeMaterial(material: THREE.Material | THREE.Material[]) {
  if (Array.isArray(material)) {
    material.forEach((item) => item.dispose())
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
