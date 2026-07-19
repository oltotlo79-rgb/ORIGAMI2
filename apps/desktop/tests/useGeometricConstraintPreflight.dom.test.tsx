import { StrictMode, type ReactNode } from 'react'
import { act, cleanup, renderHook } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { GeometricConstraintPreflightResponse } from '../src/lib/coreClient'
import { useGeometricConstraintPreflight } from '../src/lib/useGeometricConstraintPreflight'

const PROJECT_INSTANCE_ID = '10000000-0000-4000-8000-000000000001'
const PROJECT_ID = '20000000-0000-4000-8000-000000000002'

type Binding = Readonly<{
  project_instance_id: string
  project_id: string
  revision: number
}>

type Deferred<T> = Readonly<{
  promise: Promise<T>
  resolve(value: T): void
  reject(reason?: unknown): void
}>

afterEach(cleanup)

describe('useGeometricConstraintPreflight', () => {
  it('synchronously hides a completed result when the snapshot changes', async () => {
    const first = deferred<GeometricConstraintPreflightResponse>()
    const second = deferred<GeometricConstraintPreflightResponse>()
    const requests = [first, second]
    const analyze = vi.fn(() => requests.shift()!.promise)
    const snapshot1 = binding(1)
    const snapshot2 = binding(2)
    const { result, rerender } = renderHook(
      ({ snapshot }: { snapshot: Binding }) =>
        useGeometricConstraintPreflight({
          snapshot,
          enabled: true,
          analyze,
        }),
      { initialProps: { snapshot: snapshot1 } },
    )

    await resolveAndFlush(first, response(snapshot1))
    expect(result.current.preflight).toEqual(response(snapshot1))
    expect(result.current.analyzing).toBe(false)

    rerender({ snapshot: snapshot2 })

    expect(result.current.preflight).toBeNull()
    expect(result.current.analyzing).toBe(true)
    expect(result.current.failed).toBe(false)
    expect(analyze).toHaveBeenCalledTimes(2)

    await resolveAndFlush(second, response(snapshot2))
    expect(result.current.preflight).toEqual(response(snapshot2))
  })

  it('runs one request at a time, drops intermediate snapshots, and ignores stale completion', async () => {
    const first = deferred<GeometricConstraintPreflightResponse>()
    const latest = deferred<GeometricConstraintPreflightResponse>()
    const requests = [first, latest]
    let activeRequests = 0
    let maximumActiveRequests = 0
    const analyze = vi.fn((
      _projectInstanceId: string,
      _projectId: string,
      _revision: number,
    ) => {
      activeRequests += 1
      maximumActiveRequests = Math.max(maximumActiveRequests, activeRequests)
      const request = requests.shift()!
      return request.promise.then(
        (value) => {
          activeRequests -= 1
          return value
        },
        (error: unknown) => {
          activeRequests -= 1
          throw error
        },
      )
    })
    const snapshot1 = binding(1)
    const snapshot2 = binding(2)
    const snapshot3 = binding(3)
    const { result, rerender } = renderHook(
      ({ snapshot }: { snapshot: Binding }) =>
        useGeometricConstraintPreflight({
          snapshot,
          enabled: true,
          analyze,
        }),
      { initialProps: { snapshot: snapshot1 } },
    )

    expect(analyze).toHaveBeenCalledTimes(1)
    rerender({ snapshot: snapshot2 })
    expect(analyze).toHaveBeenCalledTimes(1)
    rerender({ snapshot: snapshot3 })
    expect(analyze).toHaveBeenCalledTimes(1)

    await resolveAndFlush(first, response(snapshot1))

    expect(analyze.mock.calls.map((call) => call[2])).toEqual([1, 3])
    expect(result.current.preflight).toBeNull()
    expect(result.current.analyzing).toBe(true)
    expect(maximumActiveRequests).toBe(1)

    await resolveAndFlush(latest, response(snapshot3))
    expect(result.current.preflight).toEqual(response(snapshot3))
    expect(result.current.analyzing).toBe(false)
    expect(maximumActiveRequests).toBe(1)
  })

  it('fails closed without mutating the bound snapshot', async () => {
    const request = deferred<GeometricConstraintPreflightResponse>()
    const analyze = vi.fn(() => request.promise)
    const onFailure = vi.fn()
    const snapshot = Object.freeze({
      project_instance_id: PROJECT_INSTANCE_ID,
      project_id: PROJECT_ID,
      revision: 7,
    })
    const before = structuredClone(snapshot)
    const { result } = renderHook(() =>
      useGeometricConstraintPreflight({
        snapshot,
        enabled: true,
        analyze,
        onFailure,
      }))

    await rejectAndFlush(request, new Error('native analysis failed'))

    expect(result.current.preflight).toBeNull()
    expect(result.current.analyzing).toBe(false)
    expect(result.current.failed).toBe(true)
    expect(onFailure).toHaveBeenCalledTimes(1)
    expect(snapshot).toEqual(before)
    expect(Object.isFrozen(snapshot)).toBe(true)
  })

  it('invalidates completion on unmount and never starts queued work', async () => {
    const first = deferred<GeometricConstraintPreflightResponse>()
    const analyze = vi.fn(() => first.promise)
    const onFailure = vi.fn()
    const snapshot1 = binding(1)
    const snapshot2 = binding(2)
    const { rerender, unmount } = renderHook(
      ({ snapshot }: { snapshot: Binding }) =>
        useGeometricConstraintPreflight({
          snapshot,
          enabled: true,
          analyze,
          onFailure,
        }),
      { initialProps: { snapshot: snapshot1 } },
    )

    rerender({ snapshot: snapshot2 })
    expect(analyze).toHaveBeenCalledTimes(1)
    unmount()
    await resolveAndFlush(first, response(snapshot1))

    expect(analyze).toHaveBeenCalledTimes(1)
    expect(onFailure).not.toHaveBeenCalled()
  })

  it('routes retry through the same serial lane and clears the failure immediately', async () => {
    const first = deferred<GeometricConstraintPreflightResponse>()
    const retry = deferred<GeometricConstraintPreflightResponse>()
    const requests = [first, retry]
    const analyze = vi.fn(() => requests.shift()!.promise)
    const snapshot = binding(11)
    const { result } = renderHook(() =>
      useGeometricConstraintPreflight({
        snapshot,
        enabled: true,
        analyze,
      }))

    await rejectAndFlush(first, new Error('first attempt failed'))
    expect(result.current.failed).toBe(true)

    act(() => result.current.retry())

    expect(result.current.preflight).toBeNull()
    expect(result.current.failed).toBe(false)
    expect(result.current.analyzing).toBe(true)
    expect(analyze).toHaveBeenCalledTimes(2)

    await resolveAndFlush(retry, response(snapshot))
    expect(result.current.preflight).toEqual(response(snapshot))
    expect(result.current.analyzing).toBe(false)
  })

  it('starts exactly one native request under React StrictMode', async () => {
    const request = deferred<GeometricConstraintPreflightResponse>()
    const analyze = vi.fn(() => request.promise)
    const snapshot = binding(23)
    const wrapper = ({ children }: { children: ReactNode }) => (
      <StrictMode>{children}</StrictMode>
    )
    const { result } = renderHook(
      () => useGeometricConstraintPreflight({
        snapshot,
        enabled: true,
        analyze,
      }),
      { wrapper },
    )

    expect(analyze).toHaveBeenCalledTimes(1)
    expect(result.current.preflight).toBeNull()
    expect(result.current.analyzing).toBe(true)
    expect(result.current.failed).toBe(false)

    await resolveAndFlush(request, response(snapshot))
    expect(analyze).toHaveBeenCalledTimes(1)
    expect(result.current.preflight).toEqual(response(snapshot))
    expect(result.current.analyzing).toBe(false)
  })
})

function binding(revision: number): Binding {
  return Object.freeze({
    project_instance_id: PROJECT_INSTANCE_ID,
    project_id: PROJECT_ID,
    revision,
  })
}

function response(binding: Binding): GeometricConstraintPreflightResponse {
  return Object.freeze({
    ...binding,
    result: Object.freeze({ status: 'no_direct_conflict' }),
  })
}

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })
  return Object.freeze({ promise, resolve, reject })
}

async function resolveAndFlush<T>(request: Deferred<T>, value: T) {
  await act(async () => {
    request.resolve(value)
    await request.promise
    await Promise.resolve()
    await Promise.resolve()
  })
}

async function rejectAndFlush<T>(request: Deferred<T>, error: unknown) {
  await act(async () => {
    request.reject(error)
    await request.promise.catch(() => undefined)
    await Promise.resolve()
    await Promise.resolve()
  })
}
