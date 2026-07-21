import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const app = source('../src/App.tsx')
const client = source('../src/lib/coreClient.ts')
const native = source('../src-tauri/src/beginner_recognition.rs')
const domain = source('../../../crates/ori-domain/src/beginner_recognition.rs')

test('AUT-005 recognizes only bounded CRC-checked RGBA marker PNG data', () => {
  assert.match(native, /png::Decoder/u)
  assert.match(native, /next_frame/u)
  assert.match(native, /ColorType::Rgba/u)
  assert.match(native, /BitDepth::Eight/u)
  assert.match(native, /MAX_BEGINNER_RECOGNITION_DIMENSION_V1/u)
  assert.match(native, /MAX_BEGINNER_RECOGNITION_PIXELS_V1/u)
  assert.match(domain, /BeginnerRecognitionFormatV1[\s\S]*MarkerPngV1/u)
  assert.match(domain, /collect_component/u)
})

test('recognition is bound to the project instance, revision, underlay, asset, and bytes', () => {
  assert.match(native, /ensure_expected_project/u)
  assert.match(native, /underlay\.id == request\.underlay_id && underlay\.asset == request\.asset_id/u)
  assert.match(native, /Sha256::digest\(&bytes\)/u)
  assert.match(native, /live_hash != source_sha256/u)
  assert.match(client, /exactCoreDataRecord\(value, \[[\s\S]*?'source_sha256'/u)
  assert.match(client, /record\.source_underlay_id !== expectedUnderlayId/u)
  assert.match(client, /record\.source_asset_id !== expectedAssetId/u)
})

test('the read-only proposal is stale-safe, single-flight, and copied before normal save', () => {
  assert.match(app, /beginnerRecognitionRequestRef/u)
  assert.match(app, /beginnerRecognitionBusy/u)
  assert.match(app, /latest\.revision !== binding\.revision/u)
  assert.match(app, /Recognition proposal preview/u)
  assert.match(app, /Copy to editable fields/u)
  assert.match(app, /setBeginnerSkeletonSegments/u)
  assert.match(app, /input\[name\^="target_part_"\]/u)
  assert.match(app, /read-only outline proposal/u)
  assert.match(app, /no automatic design authority/u)
  assert.match(app, /onSubmit=\{submitBeginnerDesignProfile\}/u)
})

test('bounded PNG or JPEG silhouette recognition fails closed without inferred parts', () => {
  assert.match(domain, /SilhouettePngV1/u)
  assert.match(domain, /AmbiguousSilhouette/u)
  assert.match(domain, /UnsupportedSilhouette/u)
  assert.match(domain, /target_parts: Vec::new\(\)/u)
  assert.match(native, /recognition_ambiguous_silhouette/u)
  assert.match(native, /live_hash != source_sha256/u)
  assert.match(client, /recognize_beginner_silhouette/u)
  assert.match(client, /'silhouette_png_v1'/u)
  assert.match(native, /decode_general_jpeg/u)
  assert.match(native, /MAX_BEGINNER_RECOGNITION_PIXELS_V1/u)
  assert.match(app, /Recognize outline from image/u)
  assert.match(app, /read-only outline proposal/u)
  assert.match(app, /proposal\.target_parts\.length > 0/u)
  assert.match(client, /generic_body_outline_tenths_mm\?: Array<\[number, number\]>/u)
  assert.match(client, /protrusions\?: BeginnerGenerationConstraintsV1\['protrusions'\]/u)
  assert.match(app, /setBeginnerBodyOutline\(proposal\.generic_body_outline_tenths_mm/u)
  assert.match(app, /underlay\.asset === proposal\.source_asset_id/u)
  assert.match(app, /RecognitionContourCopyAction/u)
})

test('multiple outline candidates stay strict, stale-safe, and read-only', () => {
  assert.match(client, /record\.candidates\.length > 16/u)
  assert.match(client, /'id', 'bounds', 'area_pixels', 'confidence_reason'/u)
  assert.match(client, /record\.project_instance_id !== expectedProjectInstanceId/u)
  assert.match(app, /Show outline candidates/u)
  assert.match(app, /Read-only outline candidates/u)
  assert.match(app, /They grant no generation authority/u)
  assert.match(app, /requestId === beginnerRecognitionRequestRef\.current/u)
})

test('an explicitly confirmed outline is revalidated and copied as one history command', () => {
  assert.match(native, /candidates\.get\(usize::from\(request\.candidate\.id\)\)/u)
  assert.match(native, /outline_candidate_stale/u)
  assert.match(native, /UpdateBeginnerDesignProfile/u)
  assert.match(client, /apply_beginner_outline_candidate/u)
  assert.match(client, /!confirmed \|\| !proposal\.candidates\.includes\(candidate\)/u)
  assert.match(app, /Confirm and copy to target/u)
  assert.match(app, /This does not start generation/u)
})

test('part suggestions require explicit assignment and one confirmed history command', () => {
  assert.match(native, /part_suggestion_ambiguous/u)
  assert.match(native, /part_assignment_stale/u)
  assert.match(native, /UpdateBeginnerDesignProfile/u)
  assert.match(client, /recognize_beginner_part_suggestions/u)
  assert.match(client, /apply_beginner_part_assignments/u)
  assert.match(app, /Explicit part assignments/u)
  assert.match(app, /Confirm target parts/u)
  assert.match(app, /This does not start generation/u)
  assert.match(app, /<option value="fin">/u)
  assert.match(app, /<option value="ear">/u)
  assert.match(app, /<option value="horn">/u)
  assert.match(app, /<option value="antenna">/u)
  assert.match(app, /<option value="tail">/u)
  assert.match(app, /image proves only each candidate outline/u)
  assert.match(native, /!specialized && \(2\.\.=8\)\.contains\(&feature_parts\.len\(\)\)/u)
  assert.match(native, /part_assignment_generic_binding_invalid/u)
})

test('explicit animal or insect parts feed the existing read-only symmetric plan evaluation', () => {
  assert.match(native, /BeginnerTargetCategoryV1::Insect/u)
  assert.match(native, /BeginnerTargetPartKindV1::Wing/u)
  assert.match(app, /<option value="wing">/u)
  assert.match(app, /evaluateBeginnerCandidates/u)
  assert.match(app, /Generated crease-pattern and instruction candidates/u)
  assert.match(app, /confirmAndApplyBeginnerPlan/u)
})

function source(relativePath: string) {
  return readFileSync(new URL(relativePath, import.meta.url), 'utf8')
}
