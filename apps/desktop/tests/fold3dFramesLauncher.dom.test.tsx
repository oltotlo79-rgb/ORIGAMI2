import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'

const api = vi.hoisted(() => ({
  cancel: vi.fn(),
  applyPose: vi.fn(),
  pick: vi.fn(),
  preparePose: vi.fn(),
  prepareTimeline: vi.fn(),
  applyTimeline: vi.fn(),
  select: vi.fn(),
}))

vi.mock('../src/lib/fold3dFrames.ts', () => ({
  cancelFold3dFrames: api.cancel,
  applyFold3dAppliedPose: api.applyPose,
  pickFold3dFrames: api.pick,
  prepareFold3dAppliedPose: api.preparePose,
  prepareFold3dInstructionTimeline: api.prepareTimeline,
  applyFold3dInstructionTimeline: api.applyTimeline,
  selectFold3dFrame: api.select,
}))

import { Fold3dFramesLauncher } from '../src/components/Fold3dFramesLauncher.tsx'
import { localeFixture } from './localeTestFixture.ts'

const PREVIEW = Object.freeze({
  token: '11111111-1111-4111-8111-111111111111',
  projectInstanceId: '22222222-2222-4222-8222-222222222222',
  projectId: '33333333-3333-4333-8333-333333333333',
  revision: 5,
  frameCount: 2,
  frames: Object.freeze([
    Object.freeze({
      index: 0,
      parent: null,
      inherits: false,
      vertexCount: 4,
    }),
    Object.freeze({
      index: 1,
      parent: 0,
      inherits: true,
      vertexCount: 6,
    }),
  ]),
  authorizesProjectImport: false as const,
})

const SELECTION = Object.freeze({
  token: PREVIEW.token,
  frameIndex: 0,
  vertexCount: 4,
  sourceSha256Hex: 'a'.repeat(64),
  previewImageDataUrl: 'data:image/png;base64,AA==',
  previewWidth: 512 as const,
  previewHeight: 384 as const,
  renderCoordinatesExposed: false as const,
  authorizesProjectImport: false as const,
  authorizesAppliedPose: false as const,
  authorizesInstructionTimeline: false as const,
})

const COMPATIBILITY = Object.freeze({
  token: PREVIEW.token,
  frameIndex: 0,
  hingeCount: 3,
  sourceFingerprint: 'b'.repeat(64),
  authorizesProjectGeometryMutation: false as const,
  requiresExplicitApply: true as const,
})

const TIMELINE = Object.freeze({
  token: PREVIEW.token,
  frameCount: 2,
  hingeCount: 3,
  durationMs: 1_000,
  sourceFingerprint: 'b'.repeat(64),
  geometryUnchanged: true as const,
  requiresExplicitConfirmation: true as const,
})

beforeEach(() => {
  api.cancel.mockResolvedValue(undefined)
  api.applyPose.mockResolvedValue(undefined)
  api.pick.mockResolvedValue({ canceled: false, preview: PREVIEW })
  api.preparePose.mockResolvedValue(COMPATIBILITY)
  api.prepareTimeline.mockResolvedValue(TIMELINE)
  api.applyTimeline.mockResolvedValue(undefined)
  api.select.mockResolvedValue(SELECTION)
})

afterEach(() => {
  cleanup()
  vi.clearAllMocks()
  document.body.replaceChildren()
})

it('live-translates the same preview without clearing state or invoking callbacks', async () => {
  const localeStore = localeFixture('ja')
  const onApplied = vi.fn()
  render(
    <Fold3dFramesLauncher
      disabled={false}
      onApplied={onApplied}
      localeStore={localeStore}
    />,
  )

  fireEvent.click(screen.getByRole('button', {
    name: 'FOLD 3Dフレームをプレビュー',
  }))
  const dialog = await screen.findByRole('dialog', {
    name: 'FOLD 3Dフレームプレビュー',
  })
  await waitFor(() => {
    expect(api.prepareTimeline).toHaveBeenCalledTimes(1)
  })

  const frame = screen.getByRole('combobox', {
    name: 'フレーム',
  }) as HTMLSelectElement
  const poseConfirmation = screen.getByRole('checkbox', {
    name: /現在の3D姿勢だけを置換します/u,
  }) as HTMLInputElement
  const timelineConfirmation = screen.getByRole('checkbox', {
    name: /認証済みの全frame pose/u,
  }) as HTMLInputElement
  fireEvent.click(poseConfirmation)
  fireEvent.click(timelineConfirmation)

  expect(frame.value).toBe('0')
  expect(poseConfirmation.checked).toBe(true)
  expect(timelineConfirmation.checked).toBe(true)
  expect(screen.getByRole('img', {
    name: 'フレーム 1 のネイティブプレビュー',
  })).toBeTruthy()
  expect(screen.getByText(
    '2件の完全poseを各1.0秒で一括追加します。geometryは不変で、Undo/Redoでは1件の履歴です。',
  )).toBeTruthy()

  act(() => {
    localeStore.setLocale('en')
  })

  expect(screen.getByRole('dialog', {
    name: 'FOLD 3D frame preview',
  })).toBe(dialog)
  expect(screen.getByRole('combobox', { name: 'Frame' })).toBe(frame)
  expect(screen.getByRole('checkbox', {
    name: /Replace only the current 3D pose/u,
  })).toBe(poseConfirmation)
  expect(screen.getByRole('checkbox', {
    name: /I confirm adding every authenticated frame pose/u,
  })).toBe(timelineConfirmation)
  expect(frame.value).toBe('0')
  expect(poseConfirmation.checked).toBe(true)
  expect(timelineConfirmation.checked).toBe(true)
  expect(screen.getByRole('img', {
    name: 'Native preview of frame 1',
  })).toBeTruthy()
  expect(screen.getByText(
    '2 complete poses will be appended atomically at 1.0 second each. Geometry is unchanged; Undo/Redo treats this as one history entry.',
  )).toBeTruthy()

  expect(api.pick).toHaveBeenCalledTimes(1)
  expect(api.select).toHaveBeenCalledTimes(1)
  expect(api.preparePose).toHaveBeenCalledTimes(1)
  expect(api.prepareTimeline).toHaveBeenCalledTimes(1)
  expect(api.applyPose).not.toHaveBeenCalled()
  expect(api.applyTimeline).not.toHaveBeenCalled()
  expect(api.cancel).not.toHaveBeenCalled()
  expect(onApplied).not.toHaveBeenCalled()
})
