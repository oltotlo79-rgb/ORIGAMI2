import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react'
import { useRef } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const nativeInvoke = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke: nativeInvoke }))

import { BeginnerDesignConstraints } from '../src/components/BeginnerDesignConstraints'
import {
  applyBeginnerPartAssignments,
  recognizeBeginnerPartSuggestions,
  type BeginnerRecognitionProposalV1,
  type BeginnerOutlineCandidatesResponse,
  type ProjectSnapshot,
} from '../src/lib/coreClient'
import { useBeginnerEditorState } from '../src/lib/useBeginnerEditorState'
import { useBeginnerProfileWorkflow } from '../src/lib/useBeginnerProfileWorkflow'
import { useBeginnerRecognitionWorkflow } from '../src/lib/useBeginnerRecognitionWorkflow'

const COPY = { ja: '確認', en: 'Confirm' } as const

const canvasContext = {
  clearRect: vi.fn(),
  save: vi.fn(),
  restore: vi.fn(),
  translate: vi.fn(),
  beginPath: vi.fn(),
  moveTo: vi.fn(),
  lineTo: vi.fn(),
  closePath: vi.fn(),
  stroke: vi.fn(),
  arc: vi.fn(),
  fill: vi.fn(),
  strokeStyle: '',
  lineWidth: 0,
}

function snapshot(): ProjectSnapshot {
  return {
    project_instance_id: '11111111-1111-4111-8111-111111111111',
    project_id: '22222222-2222-4222-8222-222222222222',
    revision: 4,
    fold_model_fingerprint: 'a'.repeat(64),
    beginner_design_profile: {
      schema_version: 1,
      preset: 'balanced',
      shape_fidelity_weight: 35,
      foldability_weight: 35,
      step_count_weight: 15,
      paper_efficiency_weight: 15,
      generation_constraints: {
        schema_version: 1,
        maximum_steps: 60,
        detail_level: 'standard',
        target_category: 'insect',
        target_parts: [
          { kind: 'head', count: 1 },
          { kind: 'torso', count: 1 },
          { kind: 'leg', count: 2 },
          { kind: 'horn', count: 3 },
          { kind: 'ear', count: 4 },
          { kind: 'wing', count: 5 },
          { kind: 'tail', count: 6 },
        ],
        skeleton_segments: [],
        protrusions: [],
        bulge_targets: [],
        target_asset: null,
        allowed_techniques: ['valley_fold'],
      },
    },
    underlays: {
      schema_version: 1,
      underlays: [{
        id: 'underlay-1',
        asset: 'asset-1',
        opacity: 1,
        visible: true,
        locked: false,
        transform: {
          translate_x: 0,
          translate_y: 0,
          scale_x: 1,
          scale_y: 1,
          rotation_degrees: 0,
        },
      }],
    },
  } as ProjectSnapshot
}

function markerProposal(): BeginnerRecognitionProposalV1 {
  return {
    schema_version: 1,
    format: 'marker_png_v1',
    source_underlay_id: 'underlay-1',
    source_asset_id: 'asset-1',
    source_sha256: Array(32).fill(7) as number[],
    width: 5,
    height: 1,
    shape_bounds: { min_x: 0, min_y: 0, max_x: 4, max_y: 0 },
    target_parts: [
      { kind: 'antenna', count: 2 },
      { kind: 'tail', count: 1 },
      { kind: 'fin', count: 8 },
      { kind: 'torso', count: 1 },
      { kind: 'head', count: 1 },
    ],
    skeleton_segments: [],
  }
}

function outlineCandidates(): BeginnerOutlineCandidatesResponse {
  return {
    project_instance_id: '11111111-1111-4111-8111-111111111111',
    project_id: '22222222-2222-4222-8222-222222222222',
    revision: 4,
    underlay_id: 'underlay-1',
    asset_id: 'asset-1',
    source_sha256: Array(32).fill(7),
    candidates: Array.from({ length: 16 }, (_, id) => ({
      id,
      bounds: { min_x: id * 3, min_y: 0, max_x: id * 3 + 1, max_y: 1 },
      area_pixels: 4,
      confidence_reason: id === 0 ? 'solid_component' : 'small_component',
    })),
  }
}

function partSuggestions(count: number) {
  return {
    project_instance_id: '11111111-1111-4111-8111-111111111111',
    project_id: '22222222-2222-4222-8222-222222222222',
    revision: 4,
    underlay_id: 'underlay-1',
    asset_id: 'asset-1',
    selected_outline_id: 0,
    suggestions: Array.from({ length: count }, (_, candidateId) => ({
      candidate_id: candidateId,
      suggested_kind: candidateId === 0 ? 'torso' : candidateId === 1 ? 'head' : 'leg',
      confidence_reason: candidateId === 0
        ? 'selected_primary_outline'
        : 'small_secondary_outline',
    })),
  }
}

function Harness() {
  const current = useRef(snapshot())
  const editor = useBeginnerEditorState({
    snapshot: current.current,
    getCurrentSnapshot: () => current.current,
    getSelectedFaceId: () => null,
  })
  const runNativeEdit = async (
    action: (
      projectId: string,
      revision: number,
      projectInstanceId: string,
    ) => Promise<ProjectSnapshot>,
  ) => {
    await action(
      current.current.project_id,
      current.current.revision,
      current.current.project_instance_id,
    )
    return true
  }
  const recognition = useBeginnerRecognitionWorkflow({
    snapshot: current.current,
    getCurrentSnapshot: () => current.current,
    operationBlocked: () => false,
    runNativeEdit,
    confirm: () => true,
    copy: {
      copyOutline: COPY,
      applyParts: COPY,
      copyProposal: COPY,
      overrideLowConfidence: COPY,
    },
    editor,
    onMissingReference: vi.fn(),
    onRecognitionReady: vi.fn(),
    onRecognitionFailure: vi.fn(),
    onProposalCopied: vi.fn(),
    transport: {
      recognizeTarget: vi.fn().mockResolvedValue(markerProposal()),
    } as never,
    scheduleDebounce: () => 1,
    cancelDebounce: vi.fn(),
  })
  const profile = useBeginnerProfileWorkflow({
    getCurrentSnapshot: () => current.current,
    runNativeEdit,
    editor,
    recognitionProposal: recognition.beginnerRecognitionProposal,
    silhouetteThresholds: recognition.beginnerSilhouetteThresholds,
    silhouetteCropRoi: recognition.beginnerSilhouetteCropRoi,
    silhouetteOrientation: recognition.beginnerSilhouetteOrientation,
    silhouetteMirror: recognition.beginnerSilhouetteMirror,
  })
  return (
    <form
      ref={editor.beginnerDesignFormRef}
      onSubmit={profile.submitBeginnerDesignProfile}
    >
      <input name="design_preset" value="balanced" readOnly />
      <input name="target_category" value="insect" readOnly />
      <select name="target_reference_underlay" defaultValue="underlay-1">
        <option value="underlay-1">Reference</option>
      </select>
      <BeginnerDesignConstraints
        locale="en"
        snapshot={current.current}
        coreBusy={false}
        recoveryBlocking={false}
        selectedFaceId={null}
        editor={editor}
      />
      <button
        type="button"
        onClick={() => recognition.requestBeginnerRecognition('marker')}
      >
        Recognize
      </button>
      <button
        type="button"
        onClick={recognition.copyBeginnerRecognitionProposal}
      >
        Copy proposal
      </button>
      <button type="submit">Save profile</button>
      <output data-testid="proposal-state">
        {recognition.beginnerRecognitionProposal ? 'ready' : 'empty'}
      </output>
    </form>
  )
}

function partInputs() {
  return Array.from(document.querySelectorAll<HTMLInputElement>(
    'input[name^="target_part_"]',
  ))
}

beforeEach(() => {
  nativeInvoke.mockReset()
  nativeInvoke.mockResolvedValue(snapshot())
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext')
    .mockReturnValue(canvasContext as never)
})

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
})

describe('beginner recognition part fields', () => {
  it('adds bounded Fin and Antenna fields without changing existing values', () => {
    render(<Harness />)

    const inputs = partInputs()
    expect(inputs.map((input) => input.name)).toEqual([
      'target_part_head',
      'target_part_torso',
      'target_part_leg',
      'target_part_horn',
      'target_part_ear',
      'target_part_wing',
      'target_part_fin',
      'target_part_antenna',
      'target_part_tail',
    ])
    expect(inputs.map((input) => input.value)).toEqual([
      '1',
      '1',
      '2',
      '3',
      '4',
      '5',
      '0',
      '0',
      '6',
    ])
    expect(inputs.every((input) => input.max === '8')).toBe(true)
    expect(screen.getByLabelText('Fin')).toBeTruthy()
    expect(screen.getByLabelText('Antenna')).toBeTruthy()
  })

  it('keeps recognized Fin and Antenna counts through bounded profile submit', async () => {
    render(<Harness />)

    fireEvent.click(screen.getByRole('button', { name: 'Recognize' }))
    await waitFor(() => {
      expect(screen.getByTestId('proposal-state').textContent).toBe('ready')
    })
    fireEvent.click(screen.getByRole('button', { name: 'Copy proposal' }))

    const values = new Map(partInputs().map((input) => [input.name, input.value]))
    expect(values).toEqual(new Map([
      ['target_part_head', '1'],
      ['target_part_torso', '1'],
      ['target_part_leg', '0'],
      ['target_part_horn', '0'],
      ['target_part_ear', '0'],
      ['target_part_wing', '0'],
      ['target_part_fin', '8'],
      ['target_part_antenna', '2'],
      ['target_part_tail', '1'],
    ]))

    fireEvent.submit(screen.getByRole('button', { name: 'Save profile' }).form!)
    await waitFor(() => {
      expect(nativeInvoke).toHaveBeenCalledWith(
        'update_beginner_design_profile',
        expect.objectContaining({
          profile: expect.objectContaining({
            generation_constraints: expect.objectContaining({
              target_parts: [
                { kind: 'head', count: 1 },
                { kind: 'torso', count: 1 },
                { kind: 'fin', count: 8 },
                { kind: 'antenna', count: 2 },
                { kind: 'tail', count: 1 },
              ],
            }),
          }),
        }),
      )
    })
  })

  it('rejects a recognized part count above the per-kind bound', async () => {
    render(<Harness />)

    fireEvent.click(screen.getByRole('button', { name: 'Recognize' }))
    await waitFor(() => {
      expect(screen.getByTestId('proposal-state').textContent).toBe('ready')
    })
    fireEvent.click(screen.getByRole('button', { name: 'Copy proposal' }))
    fireEvent.change(screen.getByLabelText('Fin'), {
      target: { value: '9' },
    })
    fireEvent.submit(screen.getByRole('button', { name: 'Save profile' }).form!)

    expect(nativeInvoke).not.toHaveBeenCalled()
  })
})

describe('beginner part suggestion transport bounds', () => {
  it.each([8, 9, 16])(
    'accepts a strict native suggestion response with %i records',
    async (count) => {
      const outline = outlineCandidates()
      nativeInvoke.mockResolvedValueOnce(partSuggestions(count))

      const response = await recognizeBeginnerPartSuggestions(
        outline,
        outline.candidates[0],
      )

      expect(response.suggestions).toHaveLength(count)
    },
  )

  it('rejects a seventeenth suggestion at the shared frontend cap', async () => {
    const outline = outlineCandidates()
    nativeInvoke.mockResolvedValueOnce(partSuggestions(17))

    await expect(recognizeBeginnerPartSuggestions(
      outline,
      outline.candidates[0],
    )).rejects.toMatchObject({ reason: 'native_failure' })
  })

  it('invokes apply at sixteen assignments and rejects seventeen before invoke', async () => {
    const outline = outlineCandidates()
    const assignments = Array.from({ length: 16 }, (_, candidateId) => ({
      candidate_id: candidateId,
      kind: candidateId === 0
        ? 'torso' as const
        : candidateId === 1
          ? 'head' as const
          : candidateId < 9
            ? 'leg' as const
            : 'wing' as const,
    }))

    await applyBeginnerPartAssignments(
      outline,
      outline.candidates[0],
      assignments,
    )
    expect(nativeInvoke).toHaveBeenCalledOnce()
    expect(nativeInvoke).toHaveBeenCalledWith(
      'apply_beginner_part_assignments',
      expect.any(Object),
    )

    nativeInvoke.mockClear()
    await expect(applyBeginnerPartAssignments(
      outline,
      outline.candidates[0],
      [...assignments, { candidate_id: 16, kind: 'tail' }],
    )).rejects.toMatchObject({ reason: 'native_failure' })
    expect(nativeInvoke).not.toHaveBeenCalled()
  })
})
