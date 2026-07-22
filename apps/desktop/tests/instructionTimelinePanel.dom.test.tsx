import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { createHash } from 'node:crypto'

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
  it('captures the live camera into the selected step and preserves the native save boundary', async () => {
    localeStore.setLocale('en')
    const camera = {
      position: { x: 3, y: 4, z: 5 },
      target: { x: 0.5, y: 0.25, z: 0 },
      up: { x: 0, y: 1, z: 0 },
    }
    const runNativeEdit = vi.fn(async () => SNAPSHOT)
    render(<InstructionTimelinePanel
      snapshot={SNAPSHOT}
      appliedPose={APPLIED_POSE}
      currentCamera={camera}
      poseModelKey="model-1"
      manualPoseChangeSequence={0}
      coreBusy={false}
      benchmarkActive={false}
      fileOperationActive={false}
      exportAvailable
      exportButtonRef={{ current: null }}
      animationExportButtonRef={{ current: null }}
      runNativeEdit={runNativeEdit}
      applyStepPose={vi.fn(() => true)}
      onExport={vi.fn()}
      onAnimationExport={vi.fn()}
    />)

    fireEvent.click(screen.getByRole('button', { name: /1\. Fold crane/ }))
    const capture = await screen.findByRole('button', { name: 'Capture current camera' })
    expect((capture as HTMLButtonElement).disabled).toBe(false)
    fireEvent.click(capture)

    const visual = screen.getByRole('textbox', {
      name: /^Camera, arrows, focus points, and hand guides \(JSON\)/,
    }) as HTMLTextAreaElement
    expect(JSON.parse(visual.value).camera).toEqual(camera)
    fireEvent.click(screen.getByRole('button', { name: 'Save details' }))
    await waitFor(() => expect(runNativeEdit).toHaveBeenCalledTimes(1))

    act(() => localeStore.setLocale('ja'))
    expect(screen.getByRole('button', { name: '現在のカメラを取得' })).toBeTruthy()
  })

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

    fireEvent.click(screen.getByText('1. Fold crane · 完成形サムネイル').closest('button')!)
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

    fireEvent.click(screen.getByText('1. Fold crane · Completed-form thumbnail').closest('button')!)
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
      await vi.advanceTimersByTimeAsync(101)
      await Promise.resolve()
      await vi.advanceTimersByTimeAsync(0)
    })
    const appliedIds = applyStepPose.mock.calls.map(([step]) => step.id)
    expect(appliedIds.filter((id) => id === 'physical-1').length)
      .toBeGreaterThan(1)
    expect(appliedIds.at(-1)).toBe('physical-2')
    expect(appliedIds).not.toContain('declarative')
    expect(screen.getAllByText(/手順 3「Physical two」を表示/u).length)
      .toBeGreaterThan(0)
    expect(screen.getByText('3. Physical two · 完成形サムネイル').closest('button')
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
      await vi.advanceTimersByTimeAsync(501)
      await Promise.resolve()
      await vi.advanceTimersByTimeAsync(0)
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
      await vi.advanceTimersByTimeAsync(501)
      await Promise.resolve()
      await vi.advanceTimersByTimeAsync(0)
    })
    const staleBoundaryIds = applyUntilStale.mock.calls.map(([step]) => step.id)
    expect(staleBoundaryIds.filter((id) => id === 'physical-1').length)
      .toBeGreaterThan(1)
    expect(staleBoundaryIds).not.toContain('physical-2')
    expect(screen.getAllByText(/展開図が変わった手順のため再生を停止/u).length)
      .toBeGreaterThan(0)
  })

  it('keeps the reopened proof binding visible but fails closed before endpoint verification', () => {
    const onExport = vi.fn()
    const proof = `${'7c'.repeat(32)} / 元モデル SHA-256: ${FINGERPRINT}`
    renderPanel({
      ...SNAPSHOT,
      instruction_timeline: {
        steps: [{
          ...SNAPSHOT.instruction_timeline.steps[0]!,
          description: `経路証明 SHA-256: ${proof}`,
          visual: {
            ...SNAPSHOT.instruction_timeline.steps[0]!.visual,
            path_certificate_reference_v1: {
              version: 1,
              model_id: 'bounded_certified_pose_graph_path_reference_v1',
              binding_sha256: Array(32).fill(0x7c),
              source_pose_sha256: Array(32).fill(2),
              target_pose_sha256: Array(32).fill(3),
              source_model_binding_sha256: Array(32).fill(4),
              transition_count: 1,
            },
          },
        }],
      },
    }, vi.fn(() => true), APPLIED_POSE, onExport)

    fireEvent.click(screen.getByText('1. Fold crane · 完成形サムネイル').closest('button')!)
    expect(screen.getByRole('textbox', { name: '説明' })).toHaveProperty(
      'value',
      `経路証明 SHA-256: ${proof}`,
    )
    const exportButton = screen.getByRole('button', { name: '折り図を書き出す' }) as HTMLButtonElement
    expect(exportButton.disabled).toBe(true)
    fireEvent.click(exportButton)
    expect(onExport).toHaveBeenCalledTimes(0)
  })

  it('warns instead of trusting mismatched or text-only certificate descriptions', () => {
    const baseStep = SNAPSHOT.instruction_timeline.steps[0]!
    const certificate = {
      version: 1 as const,
      model_id: 'bounded_certified_pose_graph_path_reference_v1' as const,
      binding_sha256: Array(32).fill(0x7c),
      source_pose_sha256: Array(32).fill(2),
      target_pose_sha256: Array(32).fill(3),
      source_model_binding_sha256: Array(32).fill(4),
      transition_count: 1,
    }
    const { rerender } = renderPanel({
      ...SNAPSHOT,
      instruction_timeline: {
        steps: [{
          ...baseStep,
          description: `経路証明 SHA-256: ${'8d'.repeat(32)} / 元モデル SHA-256: ${FINGERPRINT}`,
          visual: { ...baseStep.visual, path_certificate_reference_v1: certificate },
        }],
      },
    })
    fireEvent.click(screen.getByText('1. Fold crane · 完成形サムネイル').closest('button')!)
    expect(screen.getByRole('alert').textContent).toContain(
      '証明説明が構造化データと一致しません',
    )
    expect(screen.queryByLabelText('構造化経路証明')).toBeNull()

    rerender(<InstructionTimelinePanel
      snapshot={{
        ...SNAPSHOT,
        instruction_timeline: {
          steps: [{
            ...baseStep,
            description: `経路証明 SHA-256: ${'7c'.repeat(32)} / 元モデル SHA-256: ${FINGERPRINT}`,
          }],
        },
      }}
      appliedPose={APPLIED_POSE}
      poseModelKey="model-1"
      manualPoseChangeSequence={0}
      coreBusy={false}
      benchmarkActive={false}
      fileOperationActive={false}
      exportAvailable
      exportButtonRef={{ current: null }}
      animationExportButtonRef={{ current: null }}
      runNativeEdit={vi.fn(async () => SNAPSHOT)}
      applyStepPose={vi.fn(() => true)}
      onExport={vi.fn()}
      onAnimationExport={vi.fn()}
    />)
    expect(screen.getByRole('alert').textContent).toContain(
      '構造化証明データがないため、この説明文は証明として扱いません',
    )
    expect(screen.queryByLabelText('構造化経路証明')).toBeNull()
    expect((screen.getByRole('button', { name: '折り図を書き出す' }) as HTMLButtonElement).disabled)
      .toBe(false)
  })

  it('exports a certificate-free named technique only as an explicitly unproven step', () => {
    const onExport = vi.fn()
    const reopened = JSON.parse(JSON.stringify({
      ...SNAPSHOT,
      instruction_timeline: {
        steps: [{
          ...SNAPSHOT.instruction_timeline.steps[0]!,
          description: '証明参照のない名前付き技法「中割り折り」の姿勢です。連続折り経路は未証明です。',
        }],
      },
    })) as ProjectSnapshot
    renderPanel(reopened, vi.fn(() => true), APPLIED_POSE, onExport)
    fireEvent.click(screen.getByText('1. Fold crane · 完成形サムネイル').closest('button')!)
    expect(screen.getByRole('textbox', { name: '説明' })).toHaveProperty(
      'value',
      '証明参照のない名前付き技法「中割り折り」の姿勢です。連続折り経路は未証明です。',
    )
    expect(screen.queryByLabelText('構造化経路証明')).toBeNull()
    expect(screen.queryByRole('alert')).toBeNull()
    const exportButton = screen.getByRole('button', { name: '折り図を書き出す' }) as HTMLButtonElement
    expect(exportButton.disabled).toBe(false)
    fireEvent.click(exportButton)
    expect(onExport).toHaveBeenCalledTimes(1)
  })

  it('keeps atomic graph proof details aligned with PDF/SVG summary after reopen', async () => {
    const face = '11111111-1111-1111-1111-111111111111'
    const edge = '22222222-2222-2222-2222-222222222222'
    const previous = {
      ...SNAPSHOT.instruction_timeline.steps[0]!,
      id: 'proof-start',
      pose: {
        ...SNAPSHOT.instruction_timeline.steps[0]!.pose,
        fixed_face: face,
        hinge_angles: [{ edge, angle_degrees: 5 }],
      },
    }
    const targetPose = {
      ...previous.pose,
      hinge_angles: [{ edge, angle_degrees: 10 }],
    }
    const binding = Array(32).fill(0x7c)
    const reference = {
      version: 1 as const,
      model_id: 'bounded_certified_pose_graph_path_reference_v1' as const,
      binding_sha256: binding,
      source_pose_sha256: poseFingerprint(FINGERPRINT, face, previous.pose.hinge_angles),
      target_pose_sha256: poseFingerprint(FINGERPRINT, face, targetPose.hinge_angles),
      source_model_binding_sha256: sha256Bytes([
        Buffer.from('path_certificate_source_model_binding_v1'),
        Buffer.from(FINGERPRINT),
      ]),
      transition_count: 1,
    }
    const proofStep = {
      ...previous,
      id: 'proof-target',
      description: `経路証明 SHA-256: ${'7c'.repeat(32)} / 元モデル SHA-256: ${FINGERPRINT}`,
      pose: targetPose,
      visual: { ...previous.visual, path_certificate_reference_v1: reference },
    }
    const reopened = JSON.parse(JSON.stringify({
      ...SNAPSHOT,
      instruction_timeline: { steps: [previous, proofStep] },
    })) as ProjectSnapshot
    const onExport = vi.fn()
    const view = renderPanel(reopened, vi.fn(() => true), APPLIED_POSE, onExport)
    fireEvent.click(screen.getByText('2. Fold crane · 完成形サムネイル').closest('button')!)
    const proofDetails = await screen.findByLabelText('構造化経路証明')
    expect(proofDetails.textContent).toContain('証明指紋: 7c7c7c7c7c7c…')
    expect(proofDetails.textContent).toContain('検証区間: 1')
    expect(proofDetails.textContent).toContain('出力前確認（読み取り専用）')
    expect(proofDetails.textContent).toContain(`始点姿勢: ${shortBytes(reference.source_pose_sha256)}`)
    expect(proofDetails.textContent).toContain(`終点姿勢: ${shortBytes(reference.target_pose_sha256)}`)
    expect(proofDetails.textContent).toContain(
      `元モデル束縛: ${shortBytes(reference.source_model_binding_sha256)}`,
    )
    expect(proofDetails.querySelector('input, textarea, button')).toBeNull()
    const exportButton = screen.getByRole('button', { name: '折り図を書き出す' }) as HTMLButtonElement
    expect(exportButton.disabled).toBe(false)
    fireEvent.click(exportButton)
    expect(onExport).toHaveBeenCalledTimes(1)

    const graphBound = JSON.parse(JSON.stringify(reopened)) as ProjectSnapshot
    const graphReference = graphBound.instruction_timeline.steps[1]!
      .visual.path_certificate_reference_v1!
    ;(graphReference.source_pose_sha256 as number[]).splice(
      0, 32, ...graphPoseFingerprint(previous.pose.hinge_angles),
    )
    ;(graphReference.target_pose_sha256 as number[]).splice(
      0, 32, ...graphPoseFingerprint(targetPose.hinge_angles),
    )
    const exportedSummary = [
      `v1 / transitions=${graphReference.transition_count}`,
      `cert=${shortBytes(graphReference.binding_sha256).slice(0, -1)}`,
      `source=${shortBytes(graphReference.source_pose_sha256).slice(0, -1)}`,
      `target=${shortBytes(graphReference.target_pose_sha256).slice(0, -1)}`,
    ]
    view.rerender(panelFor(graphBound))
    await waitFor(() => {
      const details = screen.getByLabelText('構造化経路証明')
      expect(details.textContent).toContain(`検証区間: ${graphReference.transition_count}`)
      expect(details.textContent).toContain(`証明指紋: ${shortBytes(graphReference.binding_sha256)}`)
      expect(details.textContent).toContain(`始点姿勢: ${shortBytes(graphReference.source_pose_sha256)}`)
      expect(details.textContent).toContain(`終点姿勢: ${shortBytes(graphReference.target_pose_sha256)}`)
      expect(exportedSummary).toEqual([
        'v1 / transitions=1',
        `cert=${shortBytes(graphReference.binding_sha256).slice(0, -1)}`,
        `source=${shortBytes(graphReference.source_pose_sha256).slice(0, -1)}`,
        `target=${shortBytes(graphReference.target_pose_sha256).slice(0, -1)}`,
      ])
      expect((screen.getByRole('button', { name: '折り図を書き出す' }) as HTMLButtonElement).disabled)
        .toBe(false)
    })

    const tamperedModel = JSON.parse(JSON.stringify(reopened)) as ProjectSnapshot
    tamperedModel.instruction_timeline.steps[1]!.visual.path_certificate_reference_v1!
      .source_model_binding_sha256[0] ^= 1
    view.rerender(panelFor(tamperedModel))
    expect((screen.getByRole('button', { name: '折り図を書き出す' }) as HTMLButtonElement).disabled)
      .toBe(true)
    expect((await screen.findByRole('alert')).textContent).toContain('元モデルまたは姿勢端点')
    expect(screen.queryByLabelText('構造化経路証明')).toBeNull()
    expect((screen.getByRole('button', { name: '折り図を書き出す' }) as HTMLButtonElement).disabled)
      .toBe(true)

    const tamperedEndpoint = JSON.parse(JSON.stringify(reopened)) as ProjectSnapshot
    tamperedEndpoint.instruction_timeline.steps[1]!.visual.path_certificate_reference_v1!
      .target_pose_sha256[0] ^= 1
    view.rerender(panelFor(tamperedEndpoint))
    expect((screen.getByRole('button', { name: '折り図を書き出す' }) as HTMLButtonElement).disabled)
      .toBe(true)
    expect((await screen.findByRole('alert')).textContent).toContain('元モデルまたは姿勢端点')
    expect(screen.queryByLabelText('構造化経路証明')).toBeNull()
    expect((screen.getByRole('button', { name: '折り図を書き出す' }) as HTMLButtonElement).disabled)
      .toBe(true)
  })

  it('reopens a multi-segment Miura atomic timeline with exportable read-only proof details', async () => {
    const face = '11111111-1111-1111-1111-111111111111'
    const edge = '22222222-2222-2222-2222-222222222222'
    const poses = [5, 45, 90].map((angle) => ({
      ...SNAPSHOT.instruction_timeline.steps[0]!.pose,
      fixed_face: face,
      hinge_angles: [{ edge, angle_degrees: angle }],
    }))
    const binding = Array(32).fill(0x6d)
    const modelBinding = sha256Bytes([
      Buffer.from('path_certificate_source_model_binding_v1'),
      Buffer.from(FINGERPRINT),
    ])
    const steps = poses.map((pose, index) => ({
      ...SNAPSHOT.instruction_timeline.steps[0]!,
      id: `miura-${index}`,
      title: index === 0 ? 'Miura開始姿勢' : `Miura atomic ${index}`,
      description: index === 0
        ? '構造化証明の始点姿勢です。'
        : `認証済みの連続折り経路で「Miura atomic」を適用します。経路証明 SHA-256: ${'6d'.repeat(32)} / 元モデル SHA-256: ${FINGERPRINT}`,
      pose,
      visual: index === 0 ? SNAPSHOT.instruction_timeline.steps[0]!.visual : {
        ...SNAPSHOT.instruction_timeline.steps[0]!.visual,
        path_certificate_reference_v1: {
          version: 1 as const,
          model_id: 'bounded_certified_pose_graph_path_reference_v1' as const,
          binding_sha256: binding,
          source_pose_sha256: graphPoseFingerprint(poses[index - 1]!.hinge_angles),
          target_pose_sha256: graphPoseFingerprint(pose.hinge_angles),
          source_model_binding_sha256: modelBinding,
          transition_count: 2,
        },
      },
    }))
    const reopened = JSON.parse(JSON.stringify({
      ...SNAPSHOT,
      instruction_timeline: { steps },
    })) as ProjectSnapshot
    renderPanel(reopened)
    fireEvent.click(screen.getByText('3. Miura atomic 2 · 完成形サムネイル').closest('button')!)
    const details = await screen.findByLabelText('構造化経路証明')
    expect(details.textContent).toContain('検証区間: 2')
    expect(details.textContent).toContain('証明指紋: 6d6d6d6d6d6d…')
    expect(details.textContent).toContain(`始点姿勢: ${shortBytes(graphPoseFingerprint(poses[1]!.hinge_angles))}`)
    expect(details.textContent).toContain(`終点姿勢: ${shortBytes(graphPoseFingerprint(poses[2]!.hinge_angles))}`)
    expect(details.querySelector('input, textarea, button')).toBeNull()
    expect((screen.getByRole('button', { name: '折り図を書き出す' }) as HTMLButtonElement).disabled)
      .toBe(false)
  })
})

function panelFor(snapshot: ProjectSnapshot) {
  return <InstructionTimelinePanel
    snapshot={snapshot} appliedPose={APPLIED_POSE} poseModelKey="model-1"
    manualPoseChangeSequence={0} coreBusy={false} benchmarkActive={false}
    fileOperationActive={false} exportAvailable exportButtonRef={{ current: null }}
    animationExportButtonRef={{ current: null }} runNativeEdit={vi.fn(async () => snapshot)}
    applyStepPose={vi.fn(() => true)} onExport={vi.fn()} onAnimationExport={vi.fn()}
  />
}

function poseFingerprint(model: string, face: string, hinges: readonly { edge: string; angle_degrees: number }[]) {
  const fields: Buffer[] = [
    Buffer.from('origami2_instruction_pose_fingerprint_v1'), Buffer.from(model), uuidBuffer(face),
  ]
  for (const hinge of [...hinges].sort((a, b) => a.edge.localeCompare(b.edge))) {
    const angle = Buffer.alloc(8)
    angle.writeDoubleBE(hinge.angle_degrees)
    fields.push(uuidBuffer(hinge.edge), angle)
  }
  return sha256Bytes(fields)
}

function graphPoseFingerprint(hinges: readonly { edge: string; angle_degrees: number }[]) {
  const count = Buffer.alloc(8)
  count.writeBigUInt64BE(BigInt(hinges.length))
  const fields: Buffer[] = [Buffer.from('stacked_fold_certified_path_graph_state_v1'), count]
  for (const hinge of [...hinges].sort((a, b) => a.edge.localeCompare(b.edge))) {
    const angle = Buffer.alloc(8)
    angle.writeDoubleBE(hinge.angle_degrees)
    fields.push(uuidBuffer(hinge.edge), angle)
  }
  return sha256Bytes(fields)
}

function sha256Bytes(fields: readonly Buffer[]) {
  const hash = createHash('sha256')
  for (const field of fields) hash.update(field)
  return [...hash.digest()]
}

function shortBytes(bytes: readonly number[]) {
  return `${bytes.slice(0, 6).map((byte) => byte.toString(16).padStart(2, '0')).join('')}…`
}

function uuidBuffer(value: string) {
  return Buffer.from(value.replaceAll('-', ''), 'hex')
}

function renderPanel(
  snapshot = SNAPSHOT,
  applyStepPose = vi.fn(() => true),
  appliedPose: FoldPreviewAppliedPoseSnapshot | null = APPLIED_POSE,
  onExport = vi.fn(),
) {
  return render(
    <InstructionTimelinePanel
      snapshot={snapshot}
      appliedPose={appliedPose}
      poseModelKey="model-1"
      manualPoseChangeSequence={0}
      coreBusy={false}
      benchmarkActive={false}
      fileOperationActive={false}
      exportAvailable
      exportButtonRef={{ current: null }}
      runNativeEdit={vi.fn(async () => snapshot)}
      applyStepPose={applyStepPose}
      onExport={onExport}
    />,
  )
}
