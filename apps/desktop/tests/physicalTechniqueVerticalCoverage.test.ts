import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const instructions = readFileSync('../../crates/ori-instructions/src/technique_motion.rs', 'utf8')
const transaction = readFileSync('src-tauri/src/stacked_fold_transaction.rs', 'utf8')
const commands = readFileSync('src-tauri/src/lib.rs', 'utf8')
const client = readFileSync('src/lib/coreClient.ts', 'utf8')
const panel = readFileSync('src/components/StackedFoldPanel.tsx', 'utf8')
const editor = readFileSync('../../crates/ori-core/src/editor.rs', 'utf8')
const ori2 = readFileSync('../../crates/ori-formats/src/ori2.rs', 'utf8')
const exportSource = readFileSync('src-tauri/src/instruction_export.rs', 'utf8')
const domain = readFileSync('../../crates/ori-domain/src/lib.rs', 'utf8')
const exportLayout = readFileSync('../../crates/ori-formats/src/instruction_export/layout.rs', 'utf8')

const physicalVariants = [
  ['StraightLineStackedFold', 'book_fold', 'apply_named_book_fold_transaction'],
  ['InsideReverseFold', 'reverse_fold', 'apply_named_reverse_fold_transaction'],
  ['OutsideReverseFold', 'reverse_fold', 'apply_named_reverse_fold_transaction'],
  ['SinkFold', 'sink_fold', 'apply_named_sink_fold_transaction'],
  ['LayerSelectiveManipulation', 'layer_selective', 'apply_named_layer_selective_transaction'],
] as const

test('every physical technique variant reaches one proof-bound atomic desktop command', () => {
  for (const [rustVariant, compilerStem, command] of physicalVariants) {
    assert.match(instructions, new RegExp(`FoldTechniqueActionV1::${rustVariant}`))
    assert.match(instructions, new RegExp(`compile_certified_${compilerStem}`))
    assert.match(transaction, new RegExp(`fn ${command}`))
    assert.ok(commands.includes(command))
    assert.match(client, new RegExp(command))
  }
  assert.match(transaction, /continuous_certified\(\)/u)
  assert.match(transaction, /execute_stacked_fold_document/u)
  assert.match(transaction, /InstructionPoseModel::AbsoluteHingeAnglesV1/u)
  assert.match(panel, /applyNamedBookFoldTransaction/u)
  assert.match(panel, /applyNamedReverseFoldTransaction/u)
  assert.match(panel, /techniqueKind: namedBookFold.kind/u)
  assert.match(transaction, /compile_certified_sink_fold_timeline_v1/u)
  assert.match(transaction, /expected_preview_binding_sha256/u)
  assert.match(panel, /applyNamedLayerSelectiveTransaction/u)
})

test('the shared atomic document path owns undo redo persistence and PDF SVG export', () => {
  assert.match(editor, /execute_stacked_fold_document/u)
  assert.match(editor, /instruction_timeline: InstructionTimeline/u)
  assert.match(editor, /undo/u)
  assert.match(editor, /redo/u)
  assert.match(ori2, /instruction_timeline/u)
  assert.match(exportSource, /InstructionExportFormatRequest::Pdf/u)
  assert.match(exportSource, /InstructionExportFormatRequest::SvgZip/u)
  assert.match(exportSource, /export_instruction_document/u)
})

test('compiler kind segment and digest metadata cross persistence and both export layouts', () => {
  assert.match(domain, /NamedTechniqueCompilerMetadataV1/u)
  assert.match(domain, /compiler_output_sha256/u)
  assert.match(transaction, /bind_named_technique_compiler_metadata_v1/u)
  assert.match(client, /named_technique_compiler_v1/u)
  assert.match(exportLayout, /compiler-v1 \/ kind=/u)
  assert.match(editor, /instruction_timeline: InstructionTimeline/u)
  assert.match(ori2, /instruction_timeline/u)
})
