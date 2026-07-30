import { beforeEach, describe, expect, it, vi } from 'vitest'

const nativeInvoke = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke: nativeInvoke }))

import {
  cancelCurrentStackedFoldReadRequestV1,
  proposeCurrentCyclePoseV1,
  proposeCurrentStackedFoldRead,
  readEvenCycleCandidatesV1,
  readBoundedDyadicPoseGraphV1,
} from '../src/lib/coreClient.ts'

const instanceId = '018f47a2-4b7a-7cc1-8abc-112233445566'
const projectId = '018f47a2-4b7a-7cc1-8abc-665544332211'
const edgeId = '018f47a2-4b7a-7cc1-8abc-778899aabbcc'
const requestId = 'dyadic-graph:018f47a2-4b7a-7cc1-8abc-aabbccddeeff'

beforeEach(() => {
  nativeInvoke.mockReset()
})

describe('stacked-fold request-scoped client boundary', () => {
  it('invokes only the scoped cancellation command with the exact bounded ID', async () => {
    nativeInvoke.mockResolvedValue(undefined)

    await cancelCurrentStackedFoldReadRequestV1(requestId)

    expect(nativeInvoke).toHaveBeenCalledWith(
      'cancel_current_stacked_fold_read_request_v1',
      { requestId },
    )
  })

  it.each([
    '',
    'contains space',
    'contains\ncontrol',
    'x'.repeat(129),
  ])('rejects a non-canonical cancellation scope before IPC', async (candidate) => {
    await expect(cancelCurrentStackedFoldReadRequestV1(candidate)).rejects.toThrow(
      'invalid stacked-fold read request ID',
    )
    expect(nativeInvoke).not.toHaveBeenCalled()
  })

  it('carries the strict dyadic progress scope through the closed request', async () => {
    nativeInvoke.mockResolvedValue({
      version: 1,
      projectInstanceId: instanceId,
      projectId,
      revision: 3,
      status: 'unsupported',
      reason: 'unsupported_geometry',
      stateCount: 0,
      transitionCount: 0,
      exploredStateCount: 0,
      evaluatedTransitionCount: 0,
      certifiedTransitionCount: 0,
      certificateBindingSha256: null,
      positiveThicknessTransitionCount: 0,
      positiveThicknessCertified: false,
      positiveThicknessBindingSha256: null,
      layerTransportTransitionCount: 0,
      layerTransportCertified: false,
      layerTransportBindingSha256: null,
      mutationCandidateReady: false,
      authorizesProjectMutation: false,
    })
    const request = {
      progressRequestId: requestId,
      expectedProjectInstanceId: instanceId,
      expectedProjectId: projectId,
      expectedRevision: 3,
      targetAngles: [{ edge: edgeId, angleDegrees: 90 }],
      maxStates: 128,
      maxTransitions: 512,
      levelCount: 9 as const,
    }

    await expect(readBoundedDyadicPoseGraphV1(request)).resolves.toMatchObject({
      status: 'unsupported',
      reason: 'unsupported_geometry',
    })
    expect(nativeInvoke).toHaveBeenCalledWith(
      'read_bounded_dyadic_pose_graph_v1',
      { request },
    )
  })

  it('rejects an invalid dyadic progress scope before IPC', async () => {
    await expect(readBoundedDyadicPoseGraphV1({
      progressRequestId: 'dyadic graph has spaces',
      expectedProjectInstanceId: instanceId,
      expectedProjectId: projectId,
      expectedRevision: 3,
      targetAngles: [{ edge: edgeId, angleDegrees: 90 }],
      maxStates: 128,
      maxTransitions: 512,
      levelCount: 9,
    })).rejects.toThrow('invalid dyadic pose graph request')
    expect(nativeInvoke).not.toHaveBeenCalled()
  })

  it('uses the same non-space progress ID contract for current-cycle reads', async () => {
    const valid = {
      progressRequestId: 'current cycle has spaces',
      expectedProjectInstanceId: instanceId,
      expectedProjectId: projectId,
      expectedRevision: 3,
      cycleScheduleV1: {
        version: 2,
        entries: [],
        endpointDenominator: 1,
      },
    } as const
    await expect(proposeCurrentCyclePoseV1(valid)).rejects.toThrow(
      'invalid current-cycle preview request',
    )
    await expect(proposeCurrentCyclePoseV1({
      ...valid,
      progressRequestId: requestId,
      cycleScheduleV1: {
        ...valid.cycleScheduleV1,
        futureAuthority: false,
      },
    } as unknown as Parameters<typeof proposeCurrentCyclePoseV1>[0]))
      .rejects.toThrow('invalid current-cycle preview request')
    await expect(proposeCurrentCyclePoseV1({
      ...valid,
      progressRequestId: requestId,
      cycleScheduleV1: {
        ...valid.cycleScheduleV1,
        endpointDenominator: '1',
      },
    } as unknown as Parameters<typeof proposeCurrentCyclePoseV1>[0]))
      .rejects.toThrow('invalid current-cycle preview request')
    await expect(proposeCurrentCyclePoseV1({
      ...valid,
      progressRequestId: requestId,
      expectedRevision: -0,
    })).rejects.toThrow('invalid current-cycle preview request')
    let getterCalls = 0
    const accessor = Object.defineProperty({
      ...valid,
      progressRequestId: requestId,
    }, 'expectedRevision', {
      enumerable: true,
      get() {
        getterCalls += 1
        return 3
      },
    })
    await expect(
      proposeCurrentCyclePoseV1(
        accessor as unknown as Parameters<typeof proposeCurrentCyclePoseV1>[0],
      ),
    ).rejects.toThrow('invalid current-cycle preview request')
    expect(getterCalls).toBe(0)
    expect(nativeInvoke).not.toHaveBeenCalled()
  })

  it('deeply snapshots stacked-fold requests before IPC without invoking accessors', async () => {
    nativeInvoke.mockReturnValue(new Promise(() => undefined))
    const request = {
      progressRequestId: requestId,
      expectedProjectInstanceId: instanceId,
      expectedProjectId: projectId,
      expectedRevision: 3,
      first: [0, 0, 0] as [number, number, number],
      second: [1, 0, 0] as [number, number, number],
      fixedSide: 'left' as const,
      rotationDirection: 'positive' as const,
      requestedAngleDegrees: 90,
      linearCandidateV1: {
        version: 1 as const,
        entries: [{
          edge: edgeId,
          initialAngleDegrees: 20,
          requestedAngleDegrees: 40,
        }],
      },
    }
    void proposeCurrentStackedFoldRead(request)
    const sent = nativeInvoke.mock.calls[0]?.[1]?.request
    request.first[0] = 99
    request.linearCandidateV1.entries[0]!.requestedAngleDegrees = 80
    expect(sent.first).toEqual([0, 0, 0])
    expect(sent.linearCandidateV1.entries[0].requestedAngleDegrees).toBe(40)
    expect(Object.isFrozen(sent)).toBe(true)
    expect(Object.isFrozen(sent.linearCandidateV1.entries[0])).toBe(true)

    nativeInvoke.mockReset()
    let getterCalls = 0
    const accessor = Object.defineProperty({ ...request }, 'first', {
      enumerable: true,
      get() {
        getterCalls += 1
        return [0, 0, 0]
      },
    })
    await expect(
      proposeCurrentStackedFoldRead(accessor as typeof request),
    ).rejects.toThrow('invalid stacked-fold request')
    expect(getterCalls).toBe(0)
    expect(nativeInvoke).not.toHaveBeenCalled()
  })

  it('rejects non-canonical dyadic angles and excessive read-only pair work before IPC', async () => {
    await expect(readBoundedDyadicPoseGraphV1({
      progressRequestId: requestId,
      expectedProjectInstanceId: instanceId,
      expectedProjectId: projectId,
      expectedRevision: 3,
      targetAngles: [
        { edge: edgeId, angleDegrees: 90 },
        { edge: edgeId, angleDegrees: 80 },
      ],
      maxStates: 128,
      maxTransitions: 512,
      levelCount: 9,
    })).rejects.toThrow('invalid dyadic pose graph request')
    await expect(readEvenCycleCandidatesV1({
      expectedProjectInstanceId: instanceId,
      expectedProjectId: projectId,
      expectedRevision: 3,
      maxPairTests: 121,
    })).rejects.toThrow('invalid even-cycle candidate request')
    expect(nativeInvoke).not.toHaveBeenCalled()
  })
})
