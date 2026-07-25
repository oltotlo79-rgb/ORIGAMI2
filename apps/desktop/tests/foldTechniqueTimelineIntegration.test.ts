import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const app = [
  source('../src/App.tsx'),
  source('../src/lib/appText.ts'),
].join('\n')
const timelineHook = source('../src/lib/useFoldTechniqueTimelineProposal.ts')
const dialog = [
  source('../src/components/FoldTechniqueTimelinePreviewDialog.tsx'),
  source('../src/lib/foldTechniqueTimelinePreviewDialogText.ts'),
].join('\n')
const proposal = source('../src/lib/foldTechniqueTimelineProposal.ts')
const client = source('../src/lib/coreClient.ts')
const native = source('../src-tauri/src/lib.rs')

test('App binds the proposal to the exact project instance, revision, document, and selection', () => {
  const preview = section(
    timelineHook,
    'function previewSelected(',
    'function closePreview(',
  )
  assert.match(
    preview,
    /createFoldTechniqueTimelineProposalV1\(\s*workspace\.document,\s*input\.selectedIndex,\s*input\.locale,\s*current\.instruction_timeline\.steps\.length,\s*\)/u,
  )
  assert.match(preview, /sourceDocument: workspace\.document/u)
  assert.match(preview, /expectedProjectInstanceId: current\.project_instance_id/u)
  assert.match(preview, /expectedProjectId: current\.project_id/u)
  assert.match(preview, /expectedRevision: current\.revision/u)

  const stale = section(
    timelineHook,
    'const stale = useMemo(() => Boolean(',
    'function previewSelected(',
  )
  for (const binding of [
    'expectedProjectInstanceId',
    'expectedProjectId',
    'expectedRevision',
    'sourceDocument',
    'techniqueIndex',
  ]) assert.match(stale, new RegExp(`\\b${binding}\\b`, 'u'))
  assert.match(
    app,
    /const modalOpen = newProjectOpen[\s\S]*?\|\| foldTechniqueTimelinePreview !== null[\s\S]*?\|\| foldTechniqueTimelineBusy/u,
  )
})

test('confirmation is one atomic native edit and never dispatches a pose or physical fold', () => {
  const confirm = section(
    timelineHook,
    'async function confirmProposal()',
    'return {',
  )
  assert.match(confirm, /succeeded = await input\.runNativeEdit\(/u)
  assert.match(
    confirm,
    /appendProposal\(\s*\{\s*expectedProjectInstanceId: projectInstanceId,\s*expectedProjectId: projectId,\s*expectedRevision: revision,\s*\},\s*pending\.preview\.proposal,\s*\)/u,
  )
  assert.match(confirm, /1回のUndoで戻せます/u)
  assert.match(confirm, /One Undo removes the complete addition/u)
  assert.doesNotMatch(
    confirm,
    /addInstructionStep|replaceInstructionStepPose|applyInstructionStepPose|straightLineStackedFold/u,
  )
  assert.match(confirm, /ownedRequestActive\(requestGateRef\.current\)/u)
  assert.match(
    confirm,
    /const requestId = tryBeginOwnedRequest\(requestGateRef\.current\)/u,
  )
  assert.match(
    confirm,
    /if \(!completeOwnedRequest\(requestGateRef\.current, requestId\)\) return[\s\S]*?setBusy\(false\)/u,
  )

  const nativeCommand = section(
    native,
    'fn append_named_technique_instruction_steps(',
    '#[allow(clippy::too_many_arguments)]',
  )
  assert.match(nativeCommand, /InstructionPoseModel::DeclarativeOnlyV1/u)
  assert.match(nativeCommand, /fixed_face: None/u)
  assert.match(nativeCommand, /hinge_angles: Vec::new\(\)/u)
  assert.match(nativeCommand, /Command::AppendInstructionSteps \{ steps \}/u)
  assert.doesNotMatch(nativeCommand, /analyze_instruction_pose|finish_instruction_pose/u)
})

test('the visible review is explicit, cancel-first, busy-safe, and non-physical', () => {
  assert.match(dialog, /role="dialog"/u)
  assert.match(dialog, /aria-modal="true"/u)
  assert.match(dialog, /cancelRef\.current\?\.focus\(\)/u)
  assert.match(dialog, /if \(event\.key === 'Escape' && !busy\)/u)
  assert.match(dialog, /disabled=\{busy \|\| stale\}/u)
  assert.match(dialog, /現在の3D姿勢を変えず/u)
  assert.match(dialog, /折り重ねを含む物理コマンドを実行しません/u)
  assert.match(dialog, /Every item is description-only/u)
})

test('proposal and IPC boundaries preserve source order under fixed hard limits', () => {
  for (const sourceKind of [
    "'technique'",
    "'parameter'",
    "'precondition'",
    "'operation'",
  ]) assert.match(proposal, new RegExp(`sourceKind: ${sourceKind}`, 'u'))
  assert.match(proposal, /'source-json-v1:'/u)
  assert.match(proposal, /MAX_NAMED_TECHNIQUE_TIMELINE_PROPOSAL_STEPS = 512/u)
  assert.match(proposal, /MAX_NAMED_TECHNIQUE_TIMELINE_PROPOSAL_BYTES = 2 \* 1024 \* 1024/u)
  assert.match(proposal, /Unsupported physical operation/u)
  assert.match(proposal, /No stacked-fold physical command is executed/u)

  const clientFunction = section(
    client,
    'export function appendNamedTechniqueInstructionSteps(',
    'export function updateInstructionStepMetadata(',
  )
  assert.match(clientFunction, /isNamedTechniqueTimelineProposalV1\(proposal\)/u)
  assert.match(clientFunction, /guard: ProjectOccGuard/u)
  assert.match(clientFunction, /new TextEncoder\(\)\.encode\(proposalJson\)\.length > 2 \* 1024 \* 1024/u)
  assert.match(
    clientFunction,
    /invoke<ProjectSnapshot>\('append_named_technique_instruction_steps'/u,
  )
})

function section(value: string, start: string, end: string) {
  const startIndex = value.indexOf(start)
  assert.ok(startIndex >= 0, `missing section start: ${start}`)
  const endIndex = value.indexOf(end, startIndex + start.length)
  assert.ok(endIndex > startIndex, `missing section end: ${end}`)
  return value.slice(startIndex, endIndex)
}

function source(relativePath: string) {
  return readFileSync(new URL(relativePath, import.meta.url), 'utf8')
}
