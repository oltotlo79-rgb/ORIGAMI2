import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const nativeInvoke = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke: nativeInvoke }))

import type {
  BeginnerDesignProfileV1,
  ProjectSnapshot,
} from '../src/lib/coreClient.ts'
import { useBeginnerProfileWorkflow } from '../src/lib/useBeginnerProfileWorkflow.ts'

const INSTANCE_ID = '11111111-1111-4111-8111-111111111111'
const PROJECT_ID = '22222222-2222-4222-8222-222222222222'
const ACTIVE_MODEL = '33333333-3333-4333-8333-333333333333'
const PEER_MODEL = '44444444-4444-4444-8444-444444444444'
const ARCHIVED_MODEL = '55555555-5555-4555-8555-555555555555'
const IMAGE_ASSET = '66666666-6666-4666-8666-666666666666'
const UNDERLAY_ID = '77777777-7777-4777-8777-777777777777'

function constraints(
  targetAsset:
    BeginnerDesignProfileV1['generation_constraints']['target_asset'],
  maximumSteps = 60,
): BeginnerDesignProfileV1['generation_constraints'] {
  return {
    schema_version: 1 as const,
    maximum_steps: maximumSteps,
    detail_level: 'standard' as const,
    generic_body_outline_mode: 'symmetric' as const,
    target_category: 'animal' as const,
    target_parts: [
      { kind: 'head' as const, count: 1 },
      { kind: 'torso' as const, count: 1 },
      { kind: 'tail' as const, count: 1 },
    ],
    skeleton_segments: [],
    silhouette_thresholds: {
      schema_version: 1 as const,
      alpha: 1,
      luma: 2,
      polarity: 'dark_on_light' as const,
    },
    silhouette_orientation_degrees: 0 as const,
    silhouette_mirror: {
      schema_version: 1 as const,
      mirror_x: false,
      mirror_y: false,
    },
    protrusions: [],
    bulge_targets: [],
    target_asset: targetAsset,
    allowed_techniques: ['valley_fold' as const],
  }
}

function snapshot(
  target: 'model' | 'image',
): ProjectSnapshot {
  const targetAsset = target === 'model'
    ? { kind: 'reference_model' as const, asset_id: ACTIVE_MODEL }
    : {
        kind: 'reference_image' as const,
        underlay_id: UNDERLAY_ID,
        asset_id: IMAGE_ASSET,
      }
  return {
    project_instance_id: INSTANCE_ID,
    project_id: PROJECT_ID,
    revision: 4,
    fold_model_fingerprint: 'a'.repeat(64),
    reference_model_assets: [
      { asset_id: ACTIVE_MODEL, sha256: Array(32).fill(1) },
      { asset_id: PEER_MODEL, sha256: Array(32).fill(2) },
      { asset_id: ARCHIVED_MODEL, sha256: Array(32).fill(3) },
    ],
    underlays: {
      schema_version: 1,
      underlays: [{
        id: UNDERLAY_ID,
        asset: IMAGE_ASSET,
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
    beginner_design_profile: {
      schema_version: 1,
      preset: 'balanced',
      shape_fidelity_weight: 35,
      foldability_weight: 35,
      step_count_weight: 15,
      paper_efficiency_weight: 15,
      generation_constraints: constraints(targetAsset),
      generation_provenance: {
        schema_version: 1,
        topology_authority_sha256: Array(32).fill(9),
        confidence_score: 100,
        confidence_reasons: ['old proof'],
        explicit_override: false,
        source_asset_fingerprint: 'stale-proof',
      },
      reference_surface_landmarks_tenths_mm: [
        [10, 20, 30],
        [40, 50, 60],
      ],
      outline_edit_authority: {
        schema_version: 1,
        source_asset_id: IMAGE_ASSET,
        source_sha256: Array(32).fill(6),
        edits: [{
          kind: 'split_vertical',
          source_candidate_id: 1,
          split_x: 10,
          fragment_kinds: ['head', 'torso'],
        }],
      },
      archived_reference_model_asset_ids: [ARCHIVED_MODEL],
      reference_consensus_v1: {
        schema_version: 1,
        bindings: [
          {
            kind: 'reference_model',
            asset_id: ACTIVE_MODEL,
            sha256: Array(32).fill(1),
            quality: 100,
          },
          {
            kind: 'reference_model',
            asset_id: PEER_MODEL,
            sha256: Array(32).fill(2),
            quality: 100,
          },
        ],
      },
    },
  } as ProjectSnapshot
}

function Harness({
  current,
  submittedMaximum = 60,
}: {
  current: ProjectSnapshot
  submittedMaximum?: number
}) {
  const workflow = useBeginnerProfileWorkflow({
    getCurrentSnapshot: () => current,
    runNativeEdit: async (edit) => {
      await edit(
        current.project_id,
        current.revision,
        current.project_instance_id,
      )
      return true
    },
    editor: {
      beginnerBodyOutline: [],
      beginnerBodyOutlineMode: 'symmetric',
      beginnerSkeletonSegments: [],
      beginnerComponentBridgeOverride: null,
      beginnerProtrusions: [],
      beginnerProtrusionKinds: [],
      beginnerBulgeTargets: [],
    },
    recognitionProposal: null,
    silhouetteThresholds: {
      alpha: 1,
      luma: 2,
      polarity: 'dark_on_light',
    },
    silhouetteCropRoi: undefined,
    silhouetteOrientation: 0,
    silhouetteMirror: {
      schema_version: 1,
      mirror_x: false,
      mirror_y: false,
    },
  })
  const target =
    current.beginner_design_profile.generation_constraints.target_asset
  return (
    <form onSubmit={workflow.submitBeginnerDesignProfile}>
      <input name="design_preset" value="balanced" readOnly />
      <input
        name="maximum_steps"
        value={submittedMaximum}
        readOnly
      />
      <input name="detail_level" value="standard" readOnly />
      <input name="target_category" value="animal" readOnly />
      <input name="custom_object_display_name" value="" readOnly />
      <input name="generic_body_width_mm" value="" readOnly />
      <input name="generic_body_height_mm" value="" readOnly />
      <select
        name="target_reference_underlay"
        defaultValue={
          target?.kind === 'reference_image' ? UNDERLAY_ID : ''
        }
      >
        <option value="">None</option>
        <option value={UNDERLAY_ID}>Image</option>
      </select>
      {([
        'head',
        'torso',
        'leg',
        'horn',
        'ear',
        'wing',
        'fin',
        'antenna',
        'tail',
      ] as const).map((kind) => (
        <input
          key={kind}
          name={`target_part_${kind}`}
          value={kind === 'head' || kind === 'torso' || kind === 'tail'
            ? 1
            : 0}
          readOnly
        />
      ))}
      <input
        type="checkbox"
        name="allowed_techniques"
        value="valley_fold"
        defaultChecked
      />
      <button type="submit">Save profile</button>
    </form>
  )
}

function submittedProfile() {
  return nativeInvoke.mock.calls[0]?.[1]?.profile as
    BeginnerDesignProfileV1 | undefined
}

beforeEach(() => {
  nativeInvoke.mockReset()
  nativeInvoke.mockResolvedValue({})
})

afterEach(cleanup)

describe('beginner profile non-form evidence submission', () => {
  it('retains bookkeeping and same-live-model evidence through actual IPC', async () => {
    render(<Harness current={snapshot('model')} />)

    fireEvent.submit(screen.getByRole('button', {
      name: 'Save profile',
    }).form!)
    await waitFor(() => expect(nativeInvoke).toHaveBeenCalledOnce())

    expect(submittedProfile()).toMatchObject({
      archived_reference_model_asset_ids: [ARCHIVED_MODEL],
      reference_surface_landmarks_tenths_mm: [
        [10, 20, 30],
        [40, 50, 60],
      ],
      reference_consensus_v1: expect.objectContaining({
        schema_version: 1,
      }),
    })
    expect(submittedProfile()).not.toHaveProperty('generation_provenance')
  })

  it('clears constraint-bound authority while preserving independent model evidence', async () => {
    render(
      <Harness
        current={snapshot('model')}
        submittedMaximum={61}
      />,
    )

    fireEvent.submit(screen.getByRole('button', {
      name: 'Save profile',
    }).form!)
    await waitFor(() => expect(nativeInvoke).toHaveBeenCalledOnce())

    expect(submittedProfile()).toMatchObject({
      archived_reference_model_asset_ids: [ARCHIVED_MODEL],
      reference_surface_landmarks_tenths_mm: [
        [10, 20, 30],
        [40, 50, 60],
      ],
    })
    expect(submittedProfile()).not.toHaveProperty('generation_provenance')
    expect(submittedProfile()).not.toHaveProperty('reference_consensus_v1')
    expect(submittedProfile()).not.toHaveProperty('outline_edit_authority')
  })

  it('retains image edit authority only while its asset and constraints are exact', async () => {
    const view = render(<Harness current={snapshot('image')} />)

    fireEvent.submit(screen.getByRole('button', {
      name: 'Save profile',
    }).form!)
    await waitFor(() => expect(nativeInvoke).toHaveBeenCalledOnce())
    expect(submittedProfile()).toHaveProperty('outline_edit_authority')

    nativeInvoke.mockClear()
    view.rerender(
      <Harness
        current={snapshot('image')}
        submittedMaximum={61}
      />,
    )
    fireEvent.submit(screen.getByRole('button', {
      name: 'Save profile',
    }).form!)
    await waitFor(() => expect(nativeInvoke).toHaveBeenCalledOnce())
    expect(submittedProfile()).not.toHaveProperty('outline_edit_authority')
    expect(submittedProfile()).toMatchObject({
      archived_reference_model_asset_ids: [ARCHIVED_MODEL],
    })
  })

  it('rejects model evidence and consensus when the active target is archived', async () => {
    const current = snapshot('model')
    current.beginner_design_profile = {
      ...current.beginner_design_profile,
      archived_reference_model_asset_ids: [ACTIVE_MODEL, ARCHIVED_MODEL],
    }
    render(<Harness current={current} />)

    fireEvent.submit(screen.getByRole('button', {
      name: 'Save profile',
    }).form!)
    await waitFor(() => expect(nativeInvoke).toHaveBeenCalledOnce())

    expect(submittedProfile()).toMatchObject({
      archived_reference_model_asset_ids: [ACTIVE_MODEL, ARCHIVED_MODEL],
    })
    expect(submittedProfile()).not.toHaveProperty(
      'reference_surface_landmarks_tenths_mm',
    )
    expect(submittedProfile()).not.toHaveProperty('reference_consensus_v1')
    expect(submittedProfile()).not.toHaveProperty('generation_provenance')
  })
})
