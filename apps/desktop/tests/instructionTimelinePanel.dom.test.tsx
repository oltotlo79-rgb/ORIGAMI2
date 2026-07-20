import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { InstructionTimelinePanel } from '../src/components/InstructionTimelinePanel.tsx'
import type { ProjectSnapshot } from '../src/lib/coreClient.ts'
import type { FoldPreviewAppliedPoseSnapshot } from '../src/lib/foldPreviewAppliedPose.ts'
import { localeStore } from '../src/lib/i18n.ts'

const FINGERPRINT = 'ab'.repeat(32)
const SNAPSHOT = {
  project_instance_id: 'instance-1',
  project_id: 'project-1',
  name: 'Crane',
  current_path: null,
  revision: 4,
  saved_revision: 4,
  is_dirty: false,
  crease_pattern: { vertices: [], edges: [] },
  paper: {
    boundary_vertices: [],
    thickness_mm: 0.1,
    length_display_unit: 'mm',
    cutting_allowed: false,
    front: {
      color: { red: 255, green: 255, blue: 255, alpha: 255 },
      texture_asset: null,
    },
    back: {
      color: { red: 240, green: 240, blue: 240, alpha: 255 },
      texture_asset: null,
    },
  },
  can_undo: false,
  can_redo: false,
  cutting_allowed: false,
  instruction_timeline: {
    steps: [{
      id: 'step-1',
      title: 'Fold crane',
      description: 'Keep the edges aligned',
      caution: '',
      duration_ms: 1_500,
      visual: {
        camera: null,
        arrows: [],
        focus_points: [],
        hand_guides: [],
      },
      pose: {
        model: 'absolute_hinge_angles_v1',
        source_model_fingerprint: FINGERPRINT,
        fixed_face: null,
        hinge_angles: [],
      },
    }],
  },
  fold_model_fingerprint: FINGERPRINT,
} as ProjectSnapshot

const APPLIED_POSE: FoldPreviewAppliedPoseSnapshot = {
  projectId: SNAPSHOT.project_id,
  revision: SNAPSHOT.revision,
  fixedFaceId: null,
  hingeAngles: [],
  state: 'stable',
}

afterEach(() => {
  cleanup()
  vi.useRealTimers()
  localeStore.setLocale('ja')
  localeStore.dispose()
  document.body.replaceChildren()
  vi.restoreAllMocks()
})

describe('InstructionTimelinePanel localization', () => {
  it('translates controls, counts, durations, editor fields, and an existing notice live', async () => {
    renderPanel()

    expect(screen.getByText('折り手順')).toBeTruthy()
    expect(screen.getByText('1手順・合計 1.5秒')).toBeTruthy()
    expect(screen.getAllByText('再生停止中')).toHaveLength(2)
    expect(screen.getByRole('button', {
      name: '先頭の手順を3Dに表示',
    })).toBeTruthy()
    expect(screen.getByRole('button', {
      name: '折り図を書き出す',
    })).toBeTruthy()

    fireEvent.click(screen.getByText('1. Fold crane').closest('button')!)
    expect(await screen.findByRole('textbox', { name: 'タイトル' })).toBeTruthy()
    expect(screen.getByRole('textbox', { name: '説明' })).toBeTruthy()
    expect(screen.getByRole('button', { name: '現在の3D姿勢で更新' })).toBeTruthy()

    act(() => {
      localeStore.setLocale('en')
    })

    expect(screen.getByText('Folding instructions')).toBeTruthy()
    expect(screen.getByText('1 step · Total 1.5 seconds')).toBeTruthy()
    expect(screen.getAllByText('Playback stopped')).toHaveLength(2)
    expect(screen.getByRole('button', {
      name: 'Show the first step in 3D',
    })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Export diagrams' })).toBeTruthy()
    expect(screen.getByRole('textbox', { name: 'Title' })).toHaveProperty(
      'value',
      'Fold crane',
    )
    expect(screen.getByRole('textbox', { name: 'Description' })).toBeTruthy()
    expect(screen.getByRole('button', {
      name: 'Update with current 3D pose',
    })).toBeTruthy()
  })

  it('retranslates validation and delete confirmation messages', async () => {
    localeStore.initialize()
    localeStore.setLocale('en')
    renderPanel()

    fireEvent.click(screen.getByText('1. Fold crane').closest('button')!)
    const title = await screen.findByRole('textbox', { name: 'Title' })
    fireEvent.change(title, { target: { value: '' } })
    fireEvent.click(screen.getByRole('button', { name: 'Save details' }))
    expect(screen.getByRole('alert').textContent).toMatch(
      /title is required.*120 characters/iu,
    )

    act(() => {
      localeStore.setLocale('ja')
    })
    expect(screen.getByRole('alert').textContent).toMatch(
      /タイトルは必須.*120文字/u,
    )

    const confirm = vi.spyOn(window, 'confirm').mockReturnValue(false)
    fireEvent.click(screen.getByRole('button', { name: '削除' }))
    expect(confirm).toHaveBeenCalledWith('「Fold crane」を削除しますか？')

    act(() => {
      localeStore.setLocale('en')
    })
    fireEvent.click(screen.getByRole('button', { name: 'Delete' }))
    expect(confirm).toHaveBeenLastCalledWith('Delete “Fold crane”?')
  })

  it('shows declarative steps as explanation-only and blocks every 3D playback path', async () => {
    const declarativeSnapshot: ProjectSnapshot = {
      ...SNAPSHOT,
      instruction_timeline: {
        steps: [{
          ...SNAPSHOT.instruction_timeline.steps[0]!,
          id: 'declarative-step',
          title: '中割り折り',
          pose: {
            model: 'declarative_only_v1',
            source_model_fingerprint: 'cd'.repeat(32),
            fixed_face: null,
            hinge_angles: [],
          },
        }],
      },
    }
    const applyStepPose = vi.fn(() => true)
    renderPanel(declarativeSnapshot, applyStepPose)

    expect(screen.getByText('説明専用')).toBeTruthy()
    expect((screen.getByRole('button', {
      name: '最初の実姿勢手順を3Dに表示',
    }) as HTMLButtonElement).disabled).toBe(true)
    expect((screen.getByRole('button', {
      name: '折り図を書き出す',
    }) as HTMLButtonElement).disabled).toBe(false)

    fireEvent.click(screen.getByText('1. 中割り折り').closest('button')!)
    expect((await screen.findByRole('button', {
      name: '3Dに表示',
    }) as HTMLButtonElement).disabled).toBe(true)
    expect((screen.getByRole('button', {
      name: '現在の3D姿勢で更新',
    }) as HTMLButtonElement).disabled).toBe(true)
    expect(screen.getByText(/説明専用ステップです/u)).toBeTruthy()

    fireEvent.click(screen.getByRole('button', {
      name: '選択手順から再生',
    }))
    expect(screen.getAllByText(/3D姿勢を持たないため再生できません/u))
      .not.toHaveLength(0)
    expect(applyStepPose).not.toHaveBeenCalled()
  })

  it('plays only physical steps in a mixed timeline and keeps the original step number visible', async () => {
    vi.useFakeTimers()
    const mixedSnapshot: ProjectSnapshot = {
      ...SNAPSHOT,
      instruction_timeline: {
        steps: [
          {
            ...SNAPSHOT.instruction_timeline.steps[0]!,
            id: 'physical-1',
            title: 'Physical one',
            duration_ms: 100,
          },
          {
            ...SNAPSHOT.instruction_timeline.steps[0]!,
            id: 'declarative',
            title: 'Explanation',
            duration_ms: 100,
            pose: {
              model: 'declarative_only_v1',
              source_model_fingerprint: FINGERPRINT,
              fixed_face: null,
              hinge_angles: [],
            },
          },
          {
            ...SNAPSHOT.instruction_timeline.steps[0]!,
            id: 'physical-2',
            title: 'Physical two',
            duration_ms: 100,
          },
        ],
      },
    }
    const applyStepPose = vi.fn(() => true)
    renderPanel(mixedSnapshot, applyStepPose)

    fireEvent.click(screen.getByRole('button', {
      name: '選択手順から再生',
    }))
    await act(async () => Promise.resolve())
    expect(applyStepPose.mock.calls.map(([step]) => step.id))
      .toEqual(['physical-1'])

    await act(async () => {
      await vi.advanceTimersByTimeAsync(100)
    })
    expect(applyStepPose.mock.calls.map(([step]) => step.id))
      .toEqual(['physical-1', 'physical-2'])
    expect(screen.getAllByText(/手順 3「Physical two」を表示/u).length)
      .toBeGreaterThan(0)
    expect(screen.getByText('3. Physical two').closest('button')
      ?.getAttribute('aria-pressed')).toBe('true')
  })

  it('keeps a direct 3D route to the first physical step after a leading explanation', async () => {
    const applyStepPose = vi.fn(() => true)
    renderPanel({
      ...SNAPSHOT,
      instruction_timeline: {
        steps: [
          {
            ...SNAPSHOT.instruction_timeline.steps[0]!,
            id: 'declarative',
            title: 'Explanation',
            pose: {
              model: 'declarative_only_v1',
              source_model_fingerprint: FINGERPRINT,
              fixed_face: null,
              hinge_angles: [],
            },
          },
          {
            ...SNAPSHOT.instruction_timeline.steps[0]!,
            id: 'physical',
            title: 'Physical',
          },
        ],
      },
    }, applyStepPose)

    const showFirstPhysical = screen.getByRole('button', {
      name: '最初の実姿勢手順を3Dに表示',
    })
    expect((showFirstPhysical as HTMLButtonElement).disabled).toBe(false)
    fireEvent.click(showFirstPhysical)
    expect(applyStepPose.mock.calls.map(([step]) => step.id))
      .toEqual(['physical'])
  })

  it('does not apply a later physical step after cancellation or a stale boundary', async () => {
    vi.useFakeTimers()
    const mixedSteps: ProjectSnapshot['instruction_timeline']['steps'] = [
      {
        ...SNAPSHOT.instruction_timeline.steps[0]!,
        id: 'physical-1',
        title: 'Physical one',
        duration_ms: 100,
      },
      {
        ...SNAPSHOT.instruction_timeline.steps[0]!,
        id: 'declarative',
        title: 'Explanation',
        duration_ms: 100,
        pose: {
          model: 'declarative_only_v1',
          source_model_fingerprint: FINGERPRINT,
          fixed_face: null,
          hinge_angles: [],
        },
      },
      {
        ...SNAPSHOT.instruction_timeline.steps[0]!,
        id: 'physical-2',
        title: 'Physical two',
        duration_ms: 100,
      },
    ]
    const applyThenCancel = vi.fn(() => true)
    const { unmount } = renderPanel({
      ...SNAPSHOT,
      instruction_timeline: { steps: mixedSteps },
    }, applyThenCancel)
    fireEvent.click(screen.getByRole('button', {
      name: '選択手順から再生',
    }))
    await act(async () => Promise.resolve())
    expect(applyThenCancel).toHaveBeenCalledTimes(1)
    fireEvent.click(screen.getByRole('button', { name: '再生を停止' }))
    await act(async () => {
      await vi.advanceTimersByTimeAsync(500)
    })
    expect(applyThenCancel).toHaveBeenCalledTimes(1)
    unmount()

    const applyUntilStale = vi.fn(() => true)
    renderPanel({
      ...SNAPSHOT,
      instruction_timeline: {
        steps: mixedSteps.map((step, index) => index === 2
          ? {
              ...step,
              pose: {
                ...step.pose,
                source_model_fingerprint: 'cd'.repeat(32),
              },
            }
          : step),
      },
    }, applyUntilStale)
    fireEvent.click(screen.getByRole('button', {
      name: '選択手順から再生',
    }))
    await act(async () => Promise.resolve())
    await act(async () => {
      await vi.advanceTimersByTimeAsync(500)
    })
    expect(applyUntilStale).toHaveBeenCalledTimes(1)
    expect(screen.getAllByText(/展開図が変わった手順のため再生を停止/u).length)
      .toBeGreaterThan(0)
  })
})

function renderPanel(
  snapshot = SNAPSHOT,
  applyStepPose = vi.fn(() => true),
) {
  return render(
    <InstructionTimelinePanel
      snapshot={snapshot}
      appliedPose={APPLIED_POSE}
      poseModelKey="model-1"
      manualPoseChangeSequence={0}
      coreBusy={false}
      benchmarkActive={false}
      fileOperationActive={false}
      exportAvailable
      exportButtonRef={{ current: null }}
      runNativeEdit={vi.fn(async () => snapshot)}
      applyStepPose={applyStepPose}
      onExport={vi.fn()}
    />,
  )
}
