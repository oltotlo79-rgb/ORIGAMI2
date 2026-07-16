import { useEffect, useRef } from 'react'
import * as THREE from 'three'

export function FoldPreview({ angle }: { angle: number }) {
  const hostRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const host = hostRef.current
    if (!host) return
    const scene = new THREE.Scene()
    scene.background = new THREE.Color('#eef2f5')
    const camera = new THREE.PerspectiveCamera(36, host.clientWidth / host.clientHeight, 0.1, 100)
    camera.position.set(4.6, 3.6, 5.6)
    camera.lookAt(0, 0, 0)
    const renderer = new THREE.WebGLRenderer({ antialias: true })
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2))
    renderer.setSize(host.clientWidth, host.clientHeight)
    renderer.shadowMap.enabled = true
    host.appendChild(renderer.domElement)

    scene.add(new THREE.HemisphereLight(0xffffff, 0x748090, 2.2))
    const light = new THREE.DirectionalLight(0xffffff, 2.5)
    light.position.set(3, 6, 4)
    light.castShadow = true
    scene.add(light)
    const grid = new THREE.GridHelper(8, 16, 0xb8c1cc, 0xd7dde4)
    grid.position.y = -0.85
    scene.add(grid)

    const material = new THREE.MeshStandardMaterial({ color: '#f5a65b', side: THREE.DoubleSide, roughness: 0.72 })
    const edgeMaterial = new THREE.LineBasicMaterial({ color: '#7a3f16' })
    const leftGeometry = new THREE.PlaneGeometry(2, 3)
    const rightGeometry = new THREE.PlaneGeometry(2, 3)
    const left = new THREE.Mesh(leftGeometry, material)
    const rightPivot = new THREE.Group()
    const right = new THREE.Mesh(rightGeometry, material)
    left.rotation.x = -Math.PI / 2
    left.position.x = -1
    right.rotation.x = -Math.PI / 2
    right.position.x = 1
    rightPivot.rotation.z = THREE.MathUtils.degToRad(-angle)
    left.castShadow = right.castShadow = true
    left.receiveShadow = right.receiveShadow = true
    rightPivot.add(right)
    scene.add(left, rightPivot)
    const hinge = new THREE.Line(
      new THREE.BufferGeometry().setFromPoints([new THREE.Vector3(0, 0.02, -1.5), new THREE.Vector3(0, 0.02, 1.5)]),
      edgeMaterial,
    )
    scene.add(hinge)

    function resize() {
      if (!host) return
      camera.aspect = host.clientWidth / host.clientHeight
      camera.updateProjectionMatrix()
      renderer.setSize(host.clientWidth, host.clientHeight)
    }
    const observer = new ResizeObserver(resize)
    observer.observe(host)
    renderer.render(scene, camera)

    return () => {
      observer.disconnect()
      leftGeometry.dispose()
      rightGeometry.dispose()
      material.dispose()
      edgeMaterial.dispose()
      renderer.dispose()
      renderer.domElement.remove()
    }
  }, [angle])

  return <div ref={hostRef} className="fold-preview" data-angle={angle} />
}
