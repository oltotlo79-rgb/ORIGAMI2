import assert from 'node:assert/strict'
import test from 'node:test'
import { createInstructionOnionSkinRequest, resolveInstructionOnionSkinTransforms } from '../src/lib/instructionOnionSkin.ts'

const fingerprint = 'a'.repeat(64)
const pose = (edge: string) => ({
  model: 'absolute_hinge_angles_v1' as const,
  source_model_fingerprint: fingerprint,
  fixed_face: null,
  hinge_angles: [{ edge, angle_degrees: 30 }],
})

test('planar ghost authority rejects same-revision ABA and nonempty hinge registries', () => {
  const request = createInstructionOnionSkinRequest({
    ...base,
    steps: [{ ...step('a'), pose: { ...pose('a'), fixed_face: null, hinge_angles: [] } },
      { ...step('b'), pose: { ...pose('b'), fixed_face: null, hinge_angles: [] } }],
    selectedStepId: 'a', direction: 'next',
  })!
  const model = {
    kind: 'planar', projectId: 'project', revision: 3,
    worldUnitsPerMillimetre: 1, paperCenter: { x: 0, y: 0 },
    worldBounds: { minX: 0, minZ: 0, maxX: 1, maxZ: 1 },
    faces: [{ id: 'face', polygon: [] }], fixedFace: { id: 'face', polygon: [] },
    movingFace: null, hinge: null,
  } as any
  assert.equal(resolveInstructionOnionSkinTransforms(request, model, {
    projectInstanceId: 'foreign-instance', foldModelFingerprint: fingerprint,
  }), null)
  assert.equal(resolveInstructionOnionSkinTransforms(request, model, {
    projectInstanceId: 'instance', foldModelFingerprint: fingerprint,
  })?.size, 1)
  const tampered = { ...request, pose: { ...request.pose,
    hinge_angles: [{ edge: 'extra', angle_degrees: 1 }] } }
  assert.equal(resolveInstructionOnionSkinTransforms(tampered, model, {
    projectInstanceId: 'instance', foldModelFingerprint: fingerprint,
  }), null)
})

test('single-fold ghost resolves both fixed sides from one exact hinge', () => {
  const faceA = { id: 'face-a', polygon: [] }
  const faceB = { id: 'face-b', polygon: [] }
  const hinge = {
    edgeId: 'edge-1', leftFaceId: 'face-a', rightFaceId: 'face-b',
    start: { vertexId: 'v1', x: 0, z: 0 }, end: { vertexId: 'v2', x: 1, z: 0 },
    axis: { x: 1, z: 0 }, assignment: 'mountain', rotationSign: 1,
  }
  const model = {
    kind: 'single_fold', projectId: 'project', revision: 3,
    worldUnitsPerMillimetre: 1, paperCenter: { x: 0, y: 0 },
    worldBounds: { minX: 0, minZ: 0, maxX: 1, maxZ: 1 },
    faces: [faceA, faceB], fixedFace: faceA, movingFace: faceB, hinge,
  } as any
  for (const fixed_face of ['face-a', 'face-b']) {
    const request = {
      ...base, sourceStepId: 'source', targetStepId: 'target', direction: 'next' as const,
      pose: { ...pose('edge-1'), fixed_face },
    }
    assert.equal(resolveInstructionOnionSkinTransforms(request, model, {
      projectInstanceId: 'instance', foldModelFingerprint: fingerprint,
    })?.size, 2)
  }
})

test('tree ghost reroots a complete ordered per-hinge pose and rejects static graphs', () => {
  const faceA = { id: 'face-a', polygon: [] }
  const faceB = { id: 'face-b', polygon: [] }
  const hinge = {
    edgeId: 'edge-1', leftFaceId: 'face-a', rightFaceId: 'face-b',
    start: { vertexId: 'v1', x: 0, z: 0 }, end: { vertexId: 'v2', x: 1, z: 0 },
    axis: { x: 1, z: 0 }, assignment: 'mountain', rotationSign: 1,
  }
  const tree = { kind: 'tree', rootFaceId: 'face-a', joints: [{
    parentFaceId: 'face-a', childFaceId: 'face-b', hinge, childRotationSign: 1,
  }] }
  const model = {
    kind: 'fold_graph', projectId: 'project', revision: 3,
    worldUnitsPerMillimetre: 1, paperCenter: { x: 0, y: 0 },
    worldBounds: { minX: 0, minZ: 0, maxX: 1, maxZ: 1 },
    faces: [faceA, faceB], hinges: [hinge], kinematics: tree,
  } as any
  const request = {
    ...base, sourceStepId: 'source', targetStepId: 'target', direction: 'previous' as const,
    pose: { ...pose('edge-1'), fixed_face: 'face-b' },
  }
  assert.equal(resolveInstructionOnionSkinTransforms(request, model, {
    projectInstanceId: 'instance', foldModelFingerprint: fingerprint,
  })?.size, 2)
  for (const kind of ['static_cycle', 'static_components']) {
    assert.equal(resolveInstructionOnionSkinTransforms(request, {
      ...model, kinematics: { kind, reason: kind === 'static_cycle'
        ? 'cyclic_hinge_graph' : 'cut_material_components' },
    }, { projectInstanceId: 'instance', foldModelFingerprint: fingerprint }), null)
  }
  assert.equal(resolveInstructionOnionSkinTransforms({ ...request, direction: 'sideways' as any }, model, {
    projectInstanceId: 'instance', foldModelFingerprint: fingerprint,
  }), null)
})

test('canonical submitted hinge order is independent of reversed model registry order', () => {
  const faces = ['face-a', 'face-b', 'face-c'].map((id) => ({ id, polygon: [] }))
  const hinge = (edgeId: string, leftFaceId: string, rightFaceId: string) => ({
    edgeId, leftFaceId, rightFaceId,
    start: { vertexId: `${edgeId}-v1`, x: 0, z: 0 },
    end: { vertexId: `${edgeId}-v2`, x: 1, z: 0 }, axis: { x: 1, z: 0 },
    assignment: 'mountain', rotationSign: 1,
  })
  const first = hinge('edge-a', 'face-a', 'face-b')
  const second = hinge('edge-b', 'face-b', 'face-c')
  const model = {
    kind: 'fold_graph', projectId: 'project', revision: 3,
    worldUnitsPerMillimetre: 1, paperCenter: { x: 0, y: 0 },
    worldBounds: { minX: 0, minZ: 0, maxX: 1, maxZ: 1 }, faces,
    hinges: [second, first],
    kinematics: { kind: 'tree', rootFaceId: 'face-a', joints: [
      { parentFaceId: 'face-a', childFaceId: 'face-b', hinge: first, childRotationSign: 1 },
      { parentFaceId: 'face-b', childFaceId: 'face-c', hinge: second, childRotationSign: 1 },
    ] },
  } as any
  const request = {
    ...base, sourceStepId: 'source', targetStepId: 'target', direction: 'next' as const,
    pose: { model: 'absolute_hinge_angles_v1' as const,
      source_model_fingerprint: fingerprint, fixed_face: 'face-a',
      hinge_angles: [
        { edge: 'edge-a', angle_degrees: 10 },
        { edge: 'edge-b', angle_degrees: 20 },
      ] },
  }
  const authority = { projectInstanceId: 'instance', foldModelFingerprint: fingerprint }
  assert.equal(resolveInstructionOnionSkinTransforms(request, model, authority)?.size, 3)
  assert.equal(resolveInstructionOnionSkinTransforms({ ...request, pose: {
    ...request.pose, hinge_angles: [...request.pose.hinge_angles].reverse(),
  } }, model, authority), null)
})
const step = (id: string, options: Partial<{ stale: boolean; declarativeOnly: boolean }> = {}) => ({
  id, stale: false, declarativeOnly: false, pose: pose(id), ...options,
})
const base = {
  projectInstanceId: 'instance', projectId: 'project', revision: 3,
  foldModelFingerprint: fingerprint,
}

test('selects only the immediate previous or next physical step and detaches pose data', () => {
  const steps = [step('a'), step('b'), step('c')]
  const request = createInstructionOnionSkinRequest({ ...base, steps, selectedStepId: 'b', direction: 'next' })!
  assert.equal(request.targetStepId, 'c')
  steps[2]!.pose.hinge_angles[0]!.angle_degrees = 90
  assert.equal(request.pose.hinge_angles[0]!.angle_degrees, 30)
  assert.ok(Object.isFrozen(request.pose.hinge_angles))
})

test('does not skip declarative or stale adjacent steps and closes first/last boundaries', () => {
  assert.equal(createInstructionOnionSkinRequest({
    ...base, steps: [step('a'), step('note', { declarativeOnly: true }), step('c')],
    selectedStepId: 'a', direction: 'next',
  }), null)
  assert.equal(createInstructionOnionSkinRequest({
    ...base, steps: [step('a'), step('stale', { stale: true })],
    selectedStepId: 'a', direction: 'next',
  }), null)
  assert.equal(createInstructionOnionSkinRequest({
    ...base, steps: [step('a')], selectedStepId: 'a', direction: 'previous',
  }), null)
})
