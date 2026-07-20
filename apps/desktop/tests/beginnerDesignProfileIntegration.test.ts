import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const app = source('../src/App.tsx')
const client = source('../src/lib/coreClient.ts')
const native = source('../src-tauri/src/lib.rs')
const domain = source('../../../crates/ori-domain/src/beginner_design.rs')
const generation = source('../../../crates/ori-domain/src/beginner_generation.rs')
const editor = source('../../../crates/ori-core/src/editor.rs')
const formats = source('../../../crates/ori-formats/src/lib.rs')

test('AUT-103 and AUT-104 expose three bounded on-device scoring presets', () => {
  assert.match(domain, /BEGINNER_DESIGN_PROFILE_SCHEMA_VERSION_V1: u32 = 1/u)
  assert.match(domain, /Balanced[\s\S]*ShapePriority[\s\S]*FoldabilityPriority/u)
  assert.match(domain, /== 100/u)
  assert.match(app, /Beginner design priorities/u)
  assert.match(app, /Shape fidelity priority/u)
  assert.match(app, /Foldability priority/u)
  assert.match(app, /It does not change the current crease pattern/u)
})

test('the profile crosses strict IPC and one native history command', () => {
  assert.match(client, /normalizeBeginnerDesignProfile/u)
  assert.match(client, /exactCoreDataRecord\(value, \[[\s\S]*?'schema_version'/u)
  assert.match(client, /update_beginner_design_profile/u)
  assert.match(native, /fn update_beginner_design_profile/u)
  assert.match(native, /expected_project_instance_id/u)
  assert.match(editor, /Command::UpdateBeginnerDesignProfile/u)
  assert.match(editor, /Inverse::RestoreBeginnerDesignProfile/u)
})

test('the versioned profile is project-saved and recovery-visible', () => {
  assert.match(formats, /pub beginner_design_profile: ori_domain::BeginnerDesignProfileV1/u)
  assert.match(native, /beginner_design_profile: self\.editor\.beginner_design_profile\(\)\.clone\(\)/u)
  assert.match(native, /saved\.beginner_design_profile != \*self\.editor\.beginner_design_profile\(\)/u)
  assert.match(app, /aria-describedby="beginner-design-weights"/u)
})

test('AUT-105 generation constraints share the profile history and strict project boundary', () => {
  assert.match(generation, /pub maximum_steps: u16/u)
  assert.match(generation, /pub detail_level: BeginnerDetailLevelV1/u)
  assert.match(generation, /pub allowed_techniques: Vec<BeginnerFoldTechniqueV1>/u)
  assert.match(client, /'maximum_steps',/u)
  assert.match(client, /record\.allowed_techniques\.length > 8/u)
  assert.match(app, /name="maximum_steps"/u)
  assert.match(app, /name="detail_level"/u)
  assert.match(app, /name="allowed_techniques"/u)
  assert.match(app, /利用可能な折り技法/u)
  assert.match(app, /Allowed fold techniques/u)
})

test('AUT-001 admits only animal and insect target categories and connects them to generation', () => {
  assert.match(generation, /enum BeginnerTargetCategoryV1[\s\S]*Animal[\s\S]*Insect/u)
  assert.match(client, /record\.target_category !== 'animal'/u)
  assert.match(client, /record\.target_category !== 'insect'/u)
  assert.match(app, /name="target_category"/u)
  assert.match(app, /初版で対応する目標形状は動物と昆虫だけ/u)
  assert.match(app, /supports only animal and insect targets/u)
})

test('AUT-002 composes a bounded explicit target from supported parts', () => {
  assert.match(generation, /Head[\s\S]*Torso[\s\S]*Leg[\s\S]*Horn[\s\S]*Ear[\s\S]*Wing[\s\S]*Tail/u)
  assert.match(generation, /MAX_BEGINNER_TARGET_PARTS_TOTAL_V1: u16 = 32/u)
  assert.match(client, /record\.target_parts\.length > 7/u)
  assert.match(client, /partTotal > 32/u)
  assert.match(app, /name=\{`target_part_\$\{kind\}`\}/u)
  assert.match(app, /One head and one torso are required/u)
  assert.match(app, /Total parts: \{total\} \/ 32/u)
  assert.match(app, /候補に使用した目標部品/u)
})

test('AUT-003 stores and previews bounded stick skeleton bars with explicit dimensions', () => {
  assert.match(generation, /MAX_BEGINNER_SKELETON_SEGMENTS_V1: usize = 64/u)
  assert.match(generation, /pub thickness_tenths_mm: u16/u)
  assert.match(client, /record\.skeleton_segments\.length > 64/u)
  assert.match(client, /Math\.abs\(Number\(coordinate\)\) > 100_000/u)
  assert.match(app, /name="skeleton_length_mm"/u)
  assert.match(app, /name="skeleton_thickness_mm"/u)
  assert.match(app, /Stick skeleton preview/u)
  assert.match(app, /Only bars with explicit length and thickness are used for generation/u)
})

test('AUT-004 binds passive image and GLB references without granting generation authority', () => {
  assert.match(generation, /BeginnerTargetAssetReferenceV1/u)
  assert.match(client, /candidate\?\.kind === 'reference_image'/u)
  assert.match(client, /asset\.kind !== 'reference_model'/u)
  assert.match(client, /isCanonicalNonNilUuid\(asset\.underlay_id\)/u)
  assert.match(client, /isCanonicalNonNilUuid\(asset\.asset_id\)/u)
  assert.match(app, /name="target_reference_underlay"/u)
  assert.match(app, /Image contents are not inferred/u)
  assert.match(app, /grants no automatic recognition or fold-generation authority/u)
  assert.match(app, /selected project reference image as target input/u)
})

test('AUT-004 preview is bounded, project-bound, and stale-safe', () => {
  assert.match(client, /record\.positions\.length > 20_000/u)
  assert.match(client, /record\.triangle_indices\.length > 40_000/u)
  assert.match(client, /record\.project_instance_id !== expectedProjectInstanceId/u)
  assert.match(client, /record\.revision !== expectedRevision/u)
  assert.match(app, /beginnerReferenceRequestRef\.current/u)
  assert.match(app, /latest\.project_instance_id === geometry\.project_instance_id/u)
  assert.match(app, /Read-only 3D reference model/u)
})

test('AUT-006 stores every bounded protrusion target attribute in profile history', () => {
  assert.match(generation, /struct BeginnerProtrusionTargetV1/u)
  for (const field of [
    'count', 'length_tenths_mm', 'thickness_tenths_mm', 'position_tenths_mm',
    'direction_milli', 'symmetry', 'curvature_degrees', 'joint',
    'motion_degrees', 'side', 'priority',
  ]) assert.match(generation, new RegExp(`pub ${field}:`, 'u'))
  assert.match(generation, /MAX_BEGINNER_PROTRUSIONS_V1/u)
  assert.match(client, /'protrusions'/u)
  assert.match(client, /protrusionIds/u)
  assert.match(app, /Protrusion targets/u)
  assert.match(app, /Add protrusion target/u)
  assert.match(app, /The project is unchanged until saved/u)
  assert.match(app, /onSubmit=\{submitBeginnerDesignProfile\}/u)
  assert.match(editor, /Command::UpdateBeginnerDesignProfile/u)
  assert.match(formats, /beginner_design_profile/u)
})

test('AUT-007 binds bounded 3D face ranges and bulge direction without elasticity', () => {
  assert.match(generation, /struct BeginnerBulgeTargetV1/u)
  assert.match(generation, /pub face_ids: Vec<FaceId>/u)
  assert.match(generation, /pub range_min_tenths_mm: \[i32; 3\]/u)
  assert.match(generation, /pub direction_milli: \[i16; 3\]/u)
  assert.match(generation, /pub amount_tenths_mm: u32/u)
  assert.match(native, /bulge_targets[\s\S]*source_fold_model_fingerprint != live_fingerprint/u)
  assert.match(client, /'bulge_targets'/u)
  assert.match(app, /3D bulge targets/u)
  assert.match(app, /selectedFaceId/u)
  assert.match(app, /Elasticity is not computed/u)
})

function source(relativePath: string) {
  return readFileSync(new URL(relativePath, import.meta.url), 'utf8')
}
