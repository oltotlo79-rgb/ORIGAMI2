import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'

const { list, inspect } = vi.hoisted(() => ({
  list: vi.fn(),
  inspect: vi.fn(),
}))
vi.mock('../src/lib/coreClient.ts', () => ({
  listEffectiveCutCandidatesV1: list,
  inspectEffectiveCutReadOnlyV1: inspect,
}))

import { EffectiveCutDiagnosticPanel } from '../src/components/EffectiveCutDiagnosticPanel.tsx'
import type { ProjectSnapshot } from '../src/lib/coreClient.ts'
import { localeFixture } from './localeTestFixture.ts'

afterEach(() => {
  cleanup()
  vi.clearAllMocks()
})

it('requires explicit candidate selection and exposes no mutation action', async () => {
  list.mockResolvedValue({
    candidates: [{
      componentKey: Array(32).fill(2),
      faceCount: 1,
      areaSquareMm: 10,
      closureComponentCount: 2,
      nestedDependencyCount: 1,
    }],
  })
  inspect.mockResolvedValue({
    sourceFlatPairCount: 3,
    indeterminatePairs: 1,
    multiHingeUnionCorridorUnprovedPairs: 1,
  })
  const snapshot = {
    project_instance_id: '018f47a2-4b7a-7cc1-8abc-112233445566',
    project_id: '018f47a2-4b7a-7cc1-8abc-665544332211',
    revision: 3,
    fold_model_fingerprint: 'a'.repeat(64),
  } as ProjectSnapshot
  render(<EffectiveCutDiagnosticPanel snapshot={snapshot} />)
  const run = await screen.findByRole('button', { name: /Diagnose selection|選択を診断/ })
  expect(run.hasAttribute('disabled')).toBe(true)
  expect(screen.queryByRole('button', { name: /Apply|Save|適用|保存/ })).toBeNull()
  expect(screen.queryByText(Array(32).fill(2).join(','))).toBeNull()
  expect(screen.getByText(/\+1 (dependencies|依存成分)/)).toBeTruthy()
  fireEvent.click(screen.getByRole('checkbox'))
  expect(run.hasAttribute('disabled')).toBe(false)
  fireEvent.click(run)
  await waitFor(() => expect(inspect).toHaveBeenCalledTimes(1))
  expect(inspect.mock.calls[0][0].requestedComponentKeys).toEqual([Array(32).fill(2)])
  await screen.findByText(/Source-flat pairs|平面ペア/)
  fireEvent.click(screen.getByRole('checkbox'))
  expect(screen.queryByText(/Source-flat pairs|平面ペア/)).toBeNull()
  expect(run.hasAttribute('disabled')).toBe(true)
})

it('discards a cancelled late candidate response', async () => {
  let resolve!: (value: unknown) => void
  list.mockReturnValue(new Promise((done) => { resolve = done }))
  const snapshot = {
    project_instance_id: '018f47a2-4b7a-7cc1-8abc-112233445566',
    project_id: '018f47a2-4b7a-7cc1-8abc-665544332211',
    revision: 3,
    fold_model_fingerprint: 'a'.repeat(64),
  } as ProjectSnapshot
  render(<EffectiveCutDiagnosticPanel snapshot={snapshot} />)
  fireEvent.click(screen.getByRole('button', { name: /Cancel|キャンセル/ }))
  resolve({ candidates: [{ componentKey: Array(32).fill(2) }] })
  await Promise.resolve()
  expect(screen.queryByRole('checkbox')).toBeNull()
  expect(screen.getByRole('button', { name: /Reload candidates|候補を再取得/ })).toBeTruthy()
})

it('keeps diagnosis single-flight and discards a cancelled late result', async () => {
  list.mockResolvedValue({
    candidates: [{
      componentKey: Array(32).fill(2),
      faceCount: 1,
      areaSquareMm: 10,
      closureComponentCount: 1,
      nestedDependencyCount: 0,
    }],
  })
  let resolve!: (value: unknown) => void
  inspect.mockReturnValue(new Promise((done) => { resolve = done }))
  const snapshot = {
    project_instance_id: '018f47a2-4b7a-7cc1-8abc-112233445566',
    project_id: '018f47a2-4b7a-7cc1-8abc-665544332211',
    revision: 3,
    fold_model_fingerprint: 'a'.repeat(64),
  } as ProjectSnapshot
  render(<EffectiveCutDiagnosticPanel snapshot={snapshot} />)
  expect(await screen.findByRole('region', { name: /Effective-cut diagnostic|有効カット診断/ })).toBeTruthy()
  fireEvent.click(screen.getByRole('checkbox'))
  const run = screen.getByRole('button', { name: /Diagnose selection|選択を診断/ })
  fireEvent.click(run)
  fireEvent.click(run)
  expect(inspect).toHaveBeenCalledTimes(1)
  fireEvent.click(screen.getByRole('button', { name: /Cancel|キャンセル/ }))
  resolve({
    sourceFlatPairCount: 3,
    indeterminatePairs: 1,
    multiHingeUnionCorridorUnprovedPairs: 1,
  })
  await Promise.resolve()
  expect(screen.queryByText(/Source-flat pairs|平面ペア/)).toBeNull()
})

it('remounts on a binding key and discards the prior revision response', async () => {
  let resolveOld!: (value: unknown) => void
  list
    .mockReturnValueOnce(new Promise((done) => { resolveOld = done }))
    .mockResolvedValueOnce({
      candidates: [{
        componentKey: Array(32).fill(3),
        faceCount: 1,
        areaSquareMm: 5,
        closureComponentCount: 1,
        nestedDependencyCount: 0,
      }],
    })
  const snapshot = {
    project_instance_id: '018f47a2-4b7a-7cc1-8abc-112233445566',
    project_id: '018f47a2-4b7a-7cc1-8abc-665544332211',
    revision: 3,
    fold_model_fingerprint: 'a'.repeat(64),
  } as ProjectSnapshot
  const view = render(
    <EffectiveCutDiagnosticPanel key="binding-3" snapshot={snapshot} />,
  )
  view.rerender(
    <EffectiveCutDiagnosticPanel
      key="binding-4"
      snapshot={{ ...snapshot, revision: 4, fold_model_fingerprint: 'b'.repeat(64) }}
    />,
  )
  await screen.findByText(/1 faces|1 面/)
  resolveOld({
    candidates: [{
      componentKey: Array(32).fill(2),
      faceCount: 99,
      areaSquareMm: 50,
      closureComponentCount: 1,
      nestedDependencyCount: 0,
    }],
  })
  await Promise.resolve()
  expect(screen.queryByText(/99 faces|99 面/)).toBeNull()
})

it('provides a Japanese accessible name and read-only explanation', async () => {
  list.mockResolvedValue({
    candidates: [{
      componentKey: Array(32).fill(2),
      faceCount: 1,
      areaSquareMm: 10,
      closureComponentCount: 2,
      nestedDependencyCount: 1,
    }],
  })
  const snapshot = {
    project_instance_id: '018f47a2-4b7a-7cc1-8abc-112233445566',
    project_id: '018f47a2-4b7a-7cc1-8abc-665544332211',
    revision: 3,
    fold_model_fingerprint: 'a'.repeat(64),
  } as ProjectSnapshot
  render(<EffectiveCutDiagnosticPanel snapshot={snapshot} localeStore={localeFixture('ja')} />)
  expect(await screen.findByRole('region', { name: '有効カット診断' })).toBeTruthy()
  expect(screen.getByText('有効カット診断（読み取り専用）')).toBeTruthy()
  expect(screen.getByText(/候補の面積はその成分単体/)).toBeTruthy()
  expect(await screen.findByText(/\+1 依存成分/)).toBeTruthy()
})
