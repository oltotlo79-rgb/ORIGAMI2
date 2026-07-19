import assert from 'node:assert/strict'
import test from 'node:test'

import {
  createFoldPreviewAppliedPoseSnapshot,
  foldPreviewAppliedPoseKey,
} from '../src/lib/foldPreviewAppliedPose.ts'

test('creates a deeply detached applied-pose observation without runtime authority', () => {
  const input = {
    projectId: 'project',
    revision: 3,
    fixedFaceId: 'face',
    hingeAngles: [
      { edgeId: 'hinge-1', angleDegrees: 35 },
      { edgeId: 'hinge-2', angleDegrees: -0 },
    ],
    state: 'stable',
    privateToken: { secret: true },
  }
  const snapshot = createFoldPreviewAppliedPoseSnapshot(input)

  assert.deepEqual(snapshot, {
    projectId: 'project',
    revision: 3,
    fixedFaceId: 'face',
    hingeAngles: [
      { edgeId: 'hinge-1', angleDegrees: 35 },
      { edgeId: 'hinge-2', angleDegrees: 0 },
    ],
    state: 'stable',
  })
  assert.equal(Object.isFrozen(snapshot), true)
  assert.equal(Object.isFrozen(snapshot?.hingeAngles), true)
  input.hingeAngles[0]!.angleDegrees = 99
  assert.equal(snapshot?.hingeAngles[0]?.angleDegrees, 35)
  assert.equal('privateToken' in (snapshot ?? {}), false)
})

test('accepts all observable motion states and planar empty poses', () => {
  for (const state of ['stable', 'running', 'blocked', 'indeterminate'] as const) {
    assert.deepEqual(createFoldPreviewAppliedPoseSnapshot({
      projectId: 'project',
      revision: 0,
      fixedFaceId: null,
      hingeAngles: [],
      state,
    })?.state, state)
  }
})

test('pose identity is hinge-order independent but geometry sensitive', () => {
  const first = createFoldPreviewAppliedPoseSnapshot({
    projectId: 'project',
    revision: 3,
    fixedFaceId: 'face',
    hingeAngles: [
      { edgeId: 'hinge-b', angleDegrees: 20 },
      { edgeId: 'hinge-a', angleDegrees: 10 },
    ],
    state: 'stable',
  })
  const reorderedAndMoving = createFoldPreviewAppliedPoseSnapshot({
    projectId: 'project',
    revision: 3,
    fixedFaceId: 'face',
    hingeAngles: [
      { edgeId: 'hinge-a', angleDegrees: 10 },
      { edgeId: 'hinge-b', angleDegrees: 20 },
    ],
    state: 'running',
  })
  const changed = reorderedAndMoving && {
    ...reorderedAndMoving,
    hingeAngles: [
      { edgeId: 'hinge-a', angleDegrees: 11 },
      { edgeId: 'hinge-b', angleDegrees: 20 },
    ],
  }

  assert.equal(foldPreviewAppliedPoseKey(first), foldPreviewAppliedPoseKey(reorderedAndMoving))
  assert.notEqual(foldPreviewAppliedPoseKey(first), foldPreviewAppliedPoseKey(changed))
  assert.equal(foldPreviewAppliedPoseKey(null), null)
})

test('rejects malformed identities, revisions, angles, duplicates, and unknown states', () => {
  const valid = {
    projectId: 'project',
    revision: 0,
    fixedFaceId: 'face',
    hingeAngles: [{ edgeId: 'hinge', angleDegrees: 45 }],
    state: 'stable',
  }
  const invalid = [
    { ...valid, projectId: '' },
    { ...valid, revision: -1 },
    { ...valid, revision: 0.5 },
    { ...valid, fixedFaceId: 1 },
    { ...valid, state: 'future' },
    { ...valid, hingeAngles: [{ edgeId: '', angleDegrees: 1 }] },
    { ...valid, hingeAngles: [{ edgeId: 'hinge', angleDegrees: -1 }] },
    { ...valid, hingeAngles: [{ edgeId: 'hinge', angleDegrees: 181 }] },
    { ...valid, hingeAngles: [{ edgeId: 'hinge', angleDegrees: Number.NaN }] },
    { ...valid, hingeAngles: [
      { edgeId: 'hinge', angleDegrees: 1 },
      { edgeId: 'hinge', angleDegrees: 2 },
    ] },
  ]
  for (const value of invalid) {
    assert.equal(createFoldPreviewAppliedPoseSnapshot(value), null)
  }
})
