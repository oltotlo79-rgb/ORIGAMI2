import * as THREE from 'three'

export type FoldPreviewResolvedColor = Readonly<{
  hex: number
  opacity: number
}>

export type FoldPreviewScenePalette = Readonly<{
  frontMaterial: THREE.MeshStandardMaterial
  backMaterial: THREE.MeshStandardMaterial
  sideMaterial: THREE.MeshStandardMaterial
  paperMaterials: readonly [
    THREE.MeshStandardMaterial,
    THREE.MeshStandardMaterial,
    THREE.MeshStandardMaterial,
  ]
  edgeMaterial: THREE.LineBasicMaterial
  fixedFaceEdgeMaterial: THREE.LineBasicMaterial
  dependentFaceEdgeMaterial: THREE.LineBasicMaterial
  collisionContactEdgeMaterial: THREE.LineBasicMaterial
  collisionIndeterminateEdgeMaterial: THREE.LineBasicMaterial
  collisionPenetrationEdgeMaterial: THREE.LineBasicMaterial
}>

export type FoldPreviewSceneRuntime = Readonly<{
  scene: THREE.Scene
  camera: THREE.PerspectiveCamera
  renderer: THREE.WebGLRenderer
  palette: FoldPreviewScenePalette
  render: () => void
  resizeFromHost: () => boolean
  dispose: () => void
}>

export type FoldPreviewSceneRuntimeDependencies = Readonly<{
  createRenderer?: (
    parameters: THREE.WebGLRendererParameters,
  ) => THREE.WebGLRenderer
}>

export function createFoldPreviewSceneRuntime(
  input: Readonly<{
    host: HTMLElement
    front: FoldPreviewResolvedColor
    back: FoldPreviewResolvedColor
    devicePixelRatio: number
  }>,
  dependencies: FoldPreviewSceneRuntimeDependencies = {},
): FoldPreviewSceneRuntime {
  let renderer: THREE.WebGLRenderer | null = null
  let canvas: HTMLCanvasElement | null = null
  let grid: THREE.GridHelper | null = null
  const ownedMaterials: THREE.Material[] = []
  let disposed = false

  const dispose = () => {
    if (disposed) return
    disposed = true
    if (grid) {
      attemptCleanup(() => grid?.geometry.dispose())
      for (const material of uniqueMaterials(grid.material)) {
        attemptCleanup(() => material.dispose())
      }
    }
    for (const material of ownedMaterials) {
      attemptCleanup(() => material.dispose())
    }
    if (renderer) {
      attemptCleanup(() => renderer?.renderLists.dispose())
      attemptCleanup(() => renderer?.dispose())
      // `dispose()` releases Three.js resources but intentionally keeps the
      // browser WebGL context alive. FoldPreview owns this renderer, so losing
      // the context during teardown prevents repeated project/HMR rebuilds
      // from exhausting the browser's finite context pool.
      attemptCleanup(() => renderer?.forceContextLoss())
    }
    attemptCleanup(() => canvas?.remove())
  }

  try {
    const scene = new THREE.Scene()
    scene.background = new THREE.Color('#eef2f5')
    const initialSize = readRenderableSize(input.host)
    const camera = new THREE.PerspectiveCamera(
      36,
      initialSize ? initialSize.width / initialSize.height : 1,
      0.1,
      100,
    )
    camera.position.set(5.4, 4.7, 6.4)
    camera.lookAt(0, 0, 0)

    const createRenderer = dependencies.createRenderer
      ?? ((parameters: THREE.WebGLRendererParameters) =>
        new THREE.WebGLRenderer(parameters))
    const createdRenderer = createRenderer({ antialias: true, alpha: false })
    renderer = createdRenderer
    const createdCanvas = createdRenderer.domElement
    canvas = createdCanvas
    createdRenderer.setPixelRatio(resolveDevicePixelRatio(input.devicePixelRatio))
    createdRenderer.setSize(initialSize?.width ?? 1, initialSize?.height ?? 1, false)
    createdRenderer.outputColorSpace = THREE.SRGBColorSpace
    createdRenderer.shadowMap.enabled = true
    createdRenderer.shadowMap.type = THREE.PCFShadowMap
    createdCanvas.setAttribute('aria-hidden', 'true')
    input.host.appendChild(createdCanvas)

    scene.add(new THREE.HemisphereLight(0xffffff, 0x748090, 2.2))
    const light = new THREE.DirectionalLight(0xffffff, 2.5)
    light.position.set(3, 7, 4)
    light.castShadow = true
    scene.add(light)

    const createdGrid = new THREE.GridHelper(8, 16, 0xb8c1cc, 0xd7dde4)
    grid = createdGrid
    createdGrid.position.y = -1.35
    scene.add(createdGrid)

    const frontMaterial = ownMaterial(
      ownedMaterials,
      createPaperMaterial(input.front),
    )
    const backMaterial = ownMaterial(
      ownedMaterials,
      createPaperMaterial(input.back),
    )
    const sideMaterial = ownMaterial(
      ownedMaterials,
      new THREE.MeshStandardMaterial({
        color: mixColors(input.front.hex, input.back.hex),
        roughness: 0.82,
      }),
    )
    const paperMaterials = Object.freeze([
      frontMaterial,
      backMaterial,
      sideMaterial,
    ]) as FoldPreviewScenePalette['paperMaterials']
    const edgeMaterial = ownMaterial(
      ownedMaterials,
      new THREE.LineBasicMaterial({ color: 0x715747 }),
    )
    const fixedFaceEdgeMaterial = ownMaterial(
      ownedMaterials,
      new THREE.LineBasicMaterial({
        color: 0x1671b8,
        depthTest: false,
        depthWrite: false,
      }),
    )
    const dependentFaceEdgeMaterial = ownMaterial(
      ownedMaterials,
      new THREE.LineBasicMaterial({
        color: 0xe24a16,
        depthTest: false,
        depthWrite: false,
      }),
    )
    const collisionContactEdgeMaterial = ownMaterial(
      ownedMaterials,
      new THREE.LineBasicMaterial({
        color: 0x8e44ad,
        depthTest: false,
        depthWrite: false,
      }),
    )
    const collisionIndeterminateEdgeMaterial = ownMaterial(
      ownedMaterials,
      new THREE.LineBasicMaterial({
        color: 0xb18412,
        depthTest: false,
        depthWrite: false,
      }),
    )
    const collisionPenetrationEdgeMaterial = ownMaterial(
      ownedMaterials,
      new THREE.LineBasicMaterial({
        color: 0xc62828,
        depthTest: false,
        depthWrite: false,
      }),
    )
    const palette: FoldPreviewScenePalette = Object.freeze({
      frontMaterial,
      backMaterial,
      sideMaterial,
      paperMaterials,
      edgeMaterial,
      fixedFaceEdgeMaterial,
      dependentFaceEdgeMaterial,
      collisionContactEdgeMaterial,
      collisionIndeterminateEdgeMaterial,
      collisionPenetrationEdgeMaterial,
    })
    const render = () => {
      if (disposed) return
      createdRenderer.render(scene, camera)
    }
    const resizeFromHost = () => {
      if (disposed) return false
      const size = readRenderableSize(input.host)
      if (!size) return false
      camera.aspect = size.width / size.height
      camera.updateProjectionMatrix()
      createdRenderer.setSize(size.width, size.height, false)
      render()
      return true
    }

    return Object.freeze({
      scene,
      camera,
      renderer: createdRenderer,
      palette,
      render,
      resizeFromHost,
      dispose,
    })
  } catch (error) {
    dispose()
    throw error
  }
}

function createPaperMaterial(color: FoldPreviewResolvedColor) {
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

function ownMaterial<T extends THREE.Material>(
  materials: THREE.Material[],
  material: T,
) {
  materials.push(material)
  return material
}

function uniqueMaterials(
  material: THREE.Material | THREE.Material[],
): readonly THREE.Material[] {
  return Array.isArray(material)
    ? [...new Set(material)]
    : [material]
}

function readRenderableSize(host: HTMLElement) {
  const width = host.clientWidth
  const height = host.clientHeight
  if (!isPositiveFinite(width) || !isPositiveFinite(height)) return null
  return { width, height }
}

function resolveDevicePixelRatio(value: number) {
  return isPositiveFinite(value) ? Math.min(value, 2) : 1
}

function isPositiveFinite(value: number): value is number {
  return Number.isFinite(value) && value > 0
}

function attemptCleanup(action: () => void | undefined) {
  try {
    action()
  } catch {
    // Continue releasing the remaining independent WebGL resources.
  }
}
