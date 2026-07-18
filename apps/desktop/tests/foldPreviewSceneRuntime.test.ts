import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import * as THREE from 'three'

import {
  createFoldPreviewSceneRuntime,
  type FoldPreviewSceneRuntime,
} from '../src/lib/foldPreviewSceneRuntime.ts'

test('scene runtime preserves the preview camera, renderer, lights, grid, and palette', () => {
  const harness = fakeRendererHarness({ width: 800, height: 400 })
  let rendererParameters: THREE.WebGLRendererParameters | null = null
  const runtime = createRuntime(harness, 3, (parameters) => {
    rendererParameters = parameters
    return harness.renderer
  })

  assert.deepEqual(rendererParameters, { antialias: true, alpha: false })
  assert.deepEqual(harness.pixelRatios, [2])
  assert.deepEqual(harness.sizes, [{ width: 800, height: 400, updateStyle: false }])
  assert.equal(harness.renderer.outputColorSpace, THREE.SRGBColorSpace)
  assert.equal(harness.renderer.shadowMap.enabled, true)
  assert.equal(harness.renderer.shadowMap.type, THREE.PCFShadowMap)
  assert.equal(harness.attributes.get('aria-hidden'), 'true')
  assert.deepEqual(harness.children, [harness.sentinel, harness.canvas])

  assert.equal(runtime.camera.fov, 36)
  assert.equal(runtime.camera.aspect, 2)
  assert.equal(runtime.camera.near, 0.1)
  assert.equal(runtime.camera.far, 100)
  assert.deepEqual(runtime.camera.position.toArray(), [5.4, 4.7, 6.4])
  assert.equal((runtime.scene.background as THREE.Color).getHex(), 0xeef2f5)

  const hemisphere = runtime.scene.children.find(
    (child): child is THREE.HemisphereLight => child instanceof THREE.HemisphereLight,
  )
  assert.ok(hemisphere)
  assert.equal(hemisphere.color.getHex(), 0xffffff)
  assert.equal(hemisphere.groundColor.getHex(), 0x748090)
  assert.equal(hemisphere.intensity, 2.2)
  const directional = runtime.scene.children.find(
    (child): child is THREE.DirectionalLight => child instanceof THREE.DirectionalLight,
  )
  assert.ok(directional)
  assert.equal(directional.color.getHex(), 0xffffff)
  assert.equal(directional.intensity, 2.5)
  assert.deepEqual(directional.position.toArray(), [3, 7, 4])
  assert.equal(directional.castShadow, true)
  const grid = requiredGrid(runtime)
  assert.equal(grid.position.y, -1.35)
  const gridPositions = grid.geometry.getAttribute('position')
  const gridColors = grid.geometry.getAttribute('color')
  assert.equal(gridPositions.count, 68)
  assert.equal(gridColors.count, 68)
  assert.deepEqual(
    Array.from(gridPositions.array.slice(0, 18)),
    [
      -4, 0, -4, 4, 0, -4,
      -4, 0, -4, -4, 0, 4,
      -4, 0, -3.5, 4, 0, -3.5,
    ],
  )
  assertGridColors(gridColors, [0xb8c1cc, 0xd7dde4])
  const expectedLookDirection = runtime.camera.position.clone().negate().normalize()
  assertVectorApproximately(
    runtime.camera.getWorldDirection(new THREE.Vector3()),
    expectedLookDirection,
  )

  const palette = runtime.palette
  assert.deepEqual(palette.paperMaterials, [
    palette.frontMaterial,
    palette.backMaterial,
    palette.sideMaterial,
  ])
  assert.equal(Object.isFrozen(palette.paperMaterials), true)
  assert.equal(palette.frontMaterial.color.getHex(), 0xf5a65b)
  assert.equal(palette.frontMaterial.opacity, 0.5)
  assert.equal(palette.frontMaterial.transparent, true)
  assert.equal(palette.frontMaterial.roughness, 0.72)
  assert.equal(palette.backMaterial.color.getHex(), 0xfffdf9)
  assert.equal(palette.backMaterial.opacity, 1)
  assert.equal(palette.backMaterial.transparent, false)
  assert.equal(
    palette.sideMaterial.color.getHex(),
    new THREE.Color(0xf5a65b).lerp(new THREE.Color(0xfffdf9), 0.5).getHex(),
  )
  assert.equal(palette.sideMaterial.roughness, 0.82)
  assert.equal(palette.edgeMaterial.color.getHex(), 0x715747)
  assertEdgeMaterial(palette.fixedFaceEdgeMaterial, 0x1671b8)
  assertEdgeMaterial(palette.dependentFaceEdgeMaterial, 0xe24a16)
  assertEdgeMaterial(palette.collisionContactEdgeMaterial, 0x8e44ad)
  assertEdgeMaterial(palette.collisionIndeterminateEdgeMaterial, 0xb18412)
  assertEdgeMaterial(palette.collisionPenetrationEdgeMaterial, 0xc62828)
  assert.equal(Object.isFrozen(runtime), true)
  assert.equal(Object.isFrozen(palette), true)

  runtime.dispose()
})

test('invalid initial size and pixel ratio use the safe one-by-one fallback', () => {
  for (const devicePixelRatio of [
    Number.NaN,
    Number.POSITIVE_INFINITY,
    0,
    -1,
  ]) {
    const harness = fakeRendererHarness({ width: 0, height: Number.NaN })
    const runtime = createRuntime(harness, devicePixelRatio)
    assert.equal(runtime.camera.aspect, 1)
    assert.deepEqual(harness.pixelRatios, [1])
    assert.deepEqual(harness.sizes, [{ width: 1, height: 1, updateStyle: false }])
    runtime.dispose()
  }
})

test('render and resize delegate exact scene state without mutating invalid viewports', () => {
  const harness = fakeRendererHarness({ width: 640, height: 480 })
  const runtime = createRuntime(harness, 1.5)
  assert.deepEqual(harness.pixelRatios, [1.5])
  harness.renderCalls.length = 0
  harness.sizes.length = 0

  runtime.render()
  assert.deepEqual(harness.renderCalls, [{
    scene: runtime.scene,
    camera: runtime.camera,
  }])

  let projectionUpdates = 0
  const updateProjectionMatrix = runtime.camera.updateProjectionMatrix.bind(runtime.camera)
  runtime.camera.updateProjectionMatrix = () => {
    projectionUpdates += 1
    updateProjectionMatrix()
  }
  const previousAspect = runtime.camera.aspect
  harness.size.width = 0
  assert.equal(runtime.resizeFromHost(), false)
  assert.equal(runtime.camera.aspect, previousAspect)
  assert.equal(projectionUpdates, 0)
  assert.deepEqual(harness.sizes, [])
  assert.equal(harness.renderCalls.length, 1)

  harness.size.width = 300
  harness.size.height = 200
  assert.equal(runtime.resizeFromHost(), true)
  assert.equal(runtime.camera.aspect, 1.5)
  assert.equal(projectionUpdates, 1)
  assert.deepEqual(harness.sizes, [{
    width: 300,
    height: 200,
    updateStyle: false,
  }])
  assert.deepEqual(harness.renderCalls.at(-1), {
    scene: runtime.scene,
    camera: runtime.camera,
  })
  assert.deepEqual(harness.pixelRatios, [1.5])

  runtime.dispose()
  const renderCount = harness.renderCalls.length
  assert.equal(runtime.resizeFromHost(), false)
  runtime.render()
  assert.equal(harness.renderCalls.length, renderCount)
})

test('dispose is best-effort, idempotent, and releases only runtime-owned resources', () => {
  const harness = fakeRendererHarness({
    width: 640,
    height: 480,
    throwOnRenderListsDispose: true,
  })
  const runtime = createRuntime(harness, 1)
  const materialDisposals = new Map<THREE.Material, number>()
  const paletteMaterials = new Set<THREE.Material>([
    ...runtime.palette.paperMaterials,
    runtime.palette.edgeMaterial,
    runtime.palette.fixedFaceEdgeMaterial,
    runtime.palette.dependentFaceEdgeMaterial,
    runtime.palette.collisionContactEdgeMaterial,
    runtime.palette.collisionIndeterminateEdgeMaterial,
    runtime.palette.collisionPenetrationEdgeMaterial,
  ])
  for (const material of paletteMaterials) {
    material.dispose = () => {
      materialDisposals.set(material, (materialDisposals.get(material) ?? 0) + 1)
      if (material === runtime.palette.frontMaterial) {
        throw new Error('front material cleanup failed')
      }
    }
  }

  const grid = requiredGrid(runtime)
  let gridGeometryDisposals = 0
  grid.geometry.dispose = () => {
    gridGeometryDisposals += 1
  }
  const gridMaterials = Array.isArray(grid.material)
    ? [...new Set(grid.material)]
    : [grid.material]
  const gridMaterialDisposals = new Map<THREE.Material, number>()
  for (const material of gridMaterials) {
    material.dispose = () => {
      gridMaterialDisposals.set(
        material,
        (gridMaterialDisposals.get(material) ?? 0) + 1,
      )
    }
  }

  const callerGeometry = new THREE.BufferGeometry()
  const callerMaterial = new THREE.MeshBasicMaterial()
  let callerGeometryDisposals = 0
  let callerMaterialDisposals = 0
  callerGeometry.dispose = () => {
    callerGeometryDisposals += 1
  }
  callerMaterial.dispose = () => {
    callerMaterialDisposals += 1
  }
  runtime.scene.add(new THREE.Mesh(callerGeometry, callerMaterial))
  let replacementCanvasRemovals = 0
  const mutableRenderer = runtime.renderer as unknown as {
    domElement: { remove: () => void }
  }
  mutableRenderer.domElement = {
    remove: () => {
      replacementCanvasRemovals += 1
    },
  }

  runtime.dispose()
  runtime.dispose()

  assert.equal(gridGeometryDisposals, 1)
  for (const material of gridMaterials) {
    assert.equal(gridMaterialDisposals.get(material), 1)
  }
  assert.equal(paletteMaterials.size, 9)
  for (const material of paletteMaterials) {
    assert.equal(materialDisposals.get(material), 1)
  }
  assert.equal(harness.renderListsDisposals, 1)
  assert.equal(harness.rendererDisposals, 1)
  assert.equal(harness.contextLosses, 1)
  assert.equal(harness.canvasRemovals, 1)
  assert.equal(replacementCanvasRemovals, 0)
  assert.deepEqual(harness.children, [harness.sentinel])
  assert.equal(callerGeometryDisposals, 0)
  assert.equal(callerMaterialDisposals, 0)
})

test('construction failures roll back the exact renderer canvas and rethrow', () => {
  for (const failure of ['set_size', 'append'] as const) {
    const harness = fakeRendererHarness({
      width: 640,
      height: 480,
      throwOnSetSize: failure === 'set_size',
      throwOnAppend: failure === 'append',
      throwOnRenderListsDispose: true,
    })
    assert.throws(
      () => createRuntime(harness, 1),
      new RegExp(failure === 'set_size' ? 'set size failed' : 'append failed', 'u'),
    )
    assert.equal(harness.renderListsDisposals, 1)
    assert.equal(harness.rendererDisposals, 1)
    assert.equal(harness.contextLosses, 1)
    assert.equal(harness.canvasRemovals, 1)
    assert.deepEqual(harness.children, [harness.sentinel])
    assert.equal(
      harness.appendCalls,
      failure === 'append' ? 1 : 0,
    )
  }
})

test('a late construction failure releases the grid and already-owned palette prefix', () => {
  const harness = fakeRendererHarness({ width: 640, height: 480 })
  const originalMaterialDispose = THREE.Material.prototype.dispose
  const originalGeometryDispose = THREE.BufferGeometry.prototype.dispose
  let materialDisposals = 0
  let geometryDisposals = 0
  THREE.Material.prototype.dispose = function disposeMaterial() {
    materialDisposals += 1
    originalMaterialDispose.call(this)
  }
  THREE.BufferGeometry.prototype.dispose = function disposeGeometry() {
    geometryDisposals += 1
    originalGeometryDispose.call(this)
  }
  const input: Parameters<typeof createFoldPreviewSceneRuntime>[0] = {
    host: harness.host,
    front: { hex: 0xf5a65b, opacity: 0.5 },
    back: { hex: 0xfffdf9, opacity: 1 },
    devicePixelRatio: 1,
  }
  Object.defineProperty(input, 'back', {
    configurable: true,
    get() {
      throw new Error('back color failed')
    },
  })

  try {
    assert.throws(
      () => createFoldPreviewSceneRuntime(input, {
        createRenderer: () => harness.renderer,
      }),
      /back color failed/u,
    )
  } finally {
    THREE.Material.prototype.dispose = originalMaterialDispose
    THREE.BufferGeometry.prototype.dispose = originalGeometryDispose
  }

  assert.equal(geometryDisposals, 1)
  assert.equal(materialDisposals, 2)
  assert.equal(harness.renderListsDisposals, 1)
  assert.equal(harness.rendererDisposals, 1)
  assert.equal(harness.contextLosses, 1)
  assert.equal(harness.canvasRemovals, 1)
  assert.deepEqual(harness.children, [harness.sentinel])
})

test('render and resize failures propagate to the component teardown boundary', () => {
  const harness = fakeRendererHarness({ width: 640, height: 480 })
  const runtime = createRuntime(harness, 1)

  harness.failures.render = true
  assert.throws(() => runtime.render(), /render failed/u)
  harness.failures.render = false
  harness.failures.setSize = true
  assert.throws(() => runtime.resizeFromHost(), /set size failed/u)
  harness.failures.setSize = false
  harness.failures.render = true
  assert.throws(() => runtime.resizeFromHost(), /render failed/u)

  runtime.dispose()
})

test('scene ownership stays separate from motion, controls, and React authority', () => {
  const runtimeSource = readFileSync(
    new URL('../src/lib/foldPreviewSceneRuntime.ts', import.meta.url),
    'utf8',
  )
  const componentSource = readFileSync(
    new URL('../src/components/FoldPreview.tsx', import.meta.url),
    'utf8',
  )

  assert.match(runtimeSource, /^import \* as THREE from 'three'$/mu)
  for (const forbidden of [
    'OrbitControls',
    'ResizeObserver',
    'FoldPreviewModel',
    'MotionRuntime',
    'CorrectionAnalysis',
    'PhysicalGrab',
    'react',
    'scene.traverse',
    'replaceChildren',
    'innerHTML',
  ]) {
    assert.doesNotMatch(runtimeSource, new RegExp(forbidden, 'u'))
  }
  for (const constructor of [
    'Scene',
    'PerspectiveCamera',
    'WebGLRenderer',
    'HemisphereLight',
    'DirectionalLight',
    'GridHelper',
    'MeshStandardMaterial',
  ]) {
    assert.doesNotMatch(
      componentSource,
      new RegExp(`new THREE\\.${constructor}\\(`, 'u'),
    )
  }
  assert.equal(
    componentSource.match(/new THREE\.LineBasicMaterial\(/gu)?.length,
    2,
  )
  assert.match(componentSource, /new OrbitControls\(/u)
  assert.match(componentSource, /new ResizeObserver\(resize\)/u)
  assert.match(componentSource, /applyFoldPreviewTreeScenePose\(/u)
  assert.match(componentSource, /const materials = \[\.\.\.paperMaterials\]/u)

  const disposeStart = componentSource.indexOf('const dispose = () =>')
  const disposeEnd = componentSource.indexOf('\n    try {', disposeStart)
  assert.ok(disposeStart >= 0)
  assert.ok(disposeEnd > disposeStart)
  const disposeSource = componentSource.slice(disposeStart, disposeEnd)
  const hingeDisposal = disposeSource.indexOf('hingeMaterial?.dispose()')
  const selectedHingeDisposal = disposeSource.indexOf(
    'selectedHingeMaterial?.dispose()',
    hingeDisposal,
  )
  const runtimeRelease = disposeSource.indexOf('sceneRuntime = null', hingeDisposal)
  const runtimeDisposal = disposeSource.indexOf(
    'ownedSceneRuntime?.dispose()',
    runtimeRelease,
  )
  const controlsDisposal = disposeSource.indexOf('controls?.dispose()')
  const observerDisposal = disposeSource.indexOf('observer?.disconnect()')
  const faceGeometryDisposal = disposeSource.indexOf(
    'for (const geometry of geometries)',
  )
  const edgeGeometryDisposal = disposeSource.indexOf(
    'for (const geometry of edgeGeometries)',
  )
  const hingeGeometryDisposal = disposeSource.indexOf(
    'for (const geometry of hingeGeometries)',
  )
  assert.ok(controlsDisposal >= 0)
  assert.ok(observerDisposal > controlsDisposal)
  assert.ok(faceGeometryDisposal > observerDisposal)
  assert.ok(edgeGeometryDisposal > faceGeometryDisposal)
  assert.ok(hingeGeometryDisposal > edgeGeometryDisposal)
  assert.ok(hingeDisposal >= 0)
  assert.ok(hingeDisposal > hingeGeometryDisposal)
  assert.ok(selectedHingeDisposal > hingeDisposal)
  assert.ok(runtimeRelease > selectedHingeDisposal)
  assert.ok(runtimeDisposal > runtimeRelease)
  assert.match(
    componentSource,
    /new THREE\.LineBasicMaterial\(\{ color: 0x7a3f16 \}\)/u,
  )
  assert.match(
    componentSource,
    /color: 0xe24a16,\s+depthTest: false,\s+depthWrite: false,/u,
  )

  const resizeStart = componentSource.indexOf('const resize = () =>')
  const resizeEnd = componentSource.indexOf('observer =', resizeStart)
  const resizeSource = componentSource.slice(resizeStart, resizeEnd)
  assert.ok(resizeStart >= 0)
  assert.ok(resizeEnd > resizeStart)
  const gestureReset = resizeSource.indexOf("resetFoldGestures('reset')")
  const runtimeResize = resizeSource.indexOf('createdSceneRuntime.resizeFromHost()')
  assert.ok(gestureReset >= 0)
  assert.ok(runtimeResize > gestureReset)
})

function createRuntime(
  harness: ReturnType<typeof fakeRendererHarness>,
  devicePixelRatio: number,
  createRenderer: (
    parameters: THREE.WebGLRendererParameters,
  ) => THREE.WebGLRenderer = () => harness.renderer,
) {
  return createFoldPreviewSceneRuntime({
    host: harness.host,
    front: { hex: 0xf5a65b, opacity: 0.5 },
    back: { hex: 0xfffdf9, opacity: 1 },
    devicePixelRatio,
  }, { createRenderer })
}

function fakeRendererHarness(options: Readonly<{
  width: number
  height: number
  throwOnSetSize?: boolean
  throwOnAppend?: boolean
  throwOnRenderListsDispose?: boolean
}>) {
  const size = {
    width: options.width,
    height: options.height,
  }
  const sentinel = { kind: 'pre-existing-react-child' }
  const children: unknown[] = [sentinel]
  const attributes = new Map<string, string>()
  const pixelRatios: number[] = []
  const sizes: Array<{
    width: number
    height: number
    updateStyle: boolean
  }> = []
  const renderCalls: Array<{
    scene: THREE.Scene
    camera: THREE.Camera
  }> = []
  let appendCalls = 0
  let canvasRemovals = 0
  let renderListsDisposals = 0
  let rendererDisposals = 0
  let contextLosses = 0
  const failures = {
    setSize: false,
    render: false,
  }
  const canvas = {
    style: {},
    setAttribute(name: string, value: string) {
      attributes.set(name, value)
    },
    remove() {
      canvasRemovals += 1
      const index = children.indexOf(canvas)
      if (index >= 0) children.splice(index, 1)
    },
  }
  const host = {
    get clientWidth() {
      return size.width
    },
    get clientHeight() {
      return size.height
    },
    appendChild(child: unknown) {
      appendCalls += 1
      if (options.throwOnAppend) throw new Error('append failed')
      children.push(child)
      return child
    },
  } as unknown as HTMLElement
  const renderer = {
    domElement: canvas,
    outputColorSpace: '',
    shadowMap: {
      enabled: false,
      type: THREE.BasicShadowMap,
    },
    renderLists: {
      dispose() {
        renderListsDisposals += 1
        if (options.throwOnRenderListsDispose) {
          throw new Error('render lists cleanup failed')
        }
      },
    },
    setPixelRatio(value: number) {
      pixelRatios.push(value)
    },
    setSize(width: number, height: number, updateStyle: boolean) {
      if (options.throwOnSetSize || failures.setSize) {
        throw new Error('set size failed')
      }
      sizes.push({ width, height, updateStyle })
    },
    render(scene: THREE.Scene, camera: THREE.Camera) {
      if (failures.render) throw new Error('render failed')
      renderCalls.push({ scene, camera })
    },
    dispose() {
      rendererDisposals += 1
    },
    forceContextLoss() {
      contextLosses += 1
    },
  } as unknown as THREE.WebGLRenderer

  return {
    host,
    renderer,
    canvas,
    sentinel,
    size,
    children,
    attributes,
    pixelRatios,
    sizes,
    renderCalls,
    failures,
    get appendCalls() {
      return appendCalls
    },
    get canvasRemovals() {
      return canvasRemovals
    },
    get renderListsDisposals() {
      return renderListsDisposals
    },
    get rendererDisposals() {
      return rendererDisposals
    },
    get contextLosses() {
      return contextLosses
    },
  }
}

function requiredGrid(runtime: FoldPreviewSceneRuntime) {
  const grid = runtime.scene.children.find(
    (child): child is THREE.GridHelper => child instanceof THREE.GridHelper,
  )
  assert.ok(grid)
  return grid
}

function assertEdgeMaterial(
  material: THREE.LineBasicMaterial,
  expectedColor: number,
) {
  assert.equal(material.color.getHex(), expectedColor)
  assert.equal(material.depthTest, false)
  assert.equal(material.depthWrite, false)
}

function assertGridColors(
  attribute: THREE.BufferAttribute | THREE.InterleavedBufferAttribute,
  expectedHexColors: readonly number[],
) {
  const actual = new Set<string>()
  for (let index = 0; index < attribute.count; index += 1) {
    actual.add([
      attribute.getX(index),
      attribute.getY(index),
      attribute.getZ(index),
    ].map((value) => value.toFixed(6)).join(','))
  }
  const expected = new Set(expectedHexColors.map((hex) =>
    new THREE.Color(hex)
      .toArray()
      .map((value) => value.toFixed(6))
      .join(',')))
  assert.deepEqual(actual, expected)
}

function assertVectorApproximately(
  actual: THREE.Vector3,
  expected: THREE.Vector3,
) {
  assert.ok(actual.distanceTo(expected) <= 1e-12)
}
