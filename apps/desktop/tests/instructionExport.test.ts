import assert from 'node:assert/strict'
import test from 'node:test'

import {
  formatInstructionExportBytes,
  INSTRUCTION_EXPORT_FORMATS,
  INSTRUCTION_EXPORT_PROFILE,
  INSTRUCTION_EXPORT_PROJECTION_PROFILE,
  createInstructionExportError,
  instructionExportErrorMessage,
  instructionExportFormatLabel,
  instructionExportPhaseLabel,
  instructionExportWarningMessage,
  isInstructionExportFormat,
  type InstructionExportPreviewResponse,
  type InstructionExportSaveResponse,
} from '../src/lib/instructionExport.ts'

test('instruction export formats are a closed PDF/SVG ZIP set with stable labels', () => {
  assert.deepEqual(
    INSTRUCTION_EXPORT_FORMATS.map(({ value }) => value),
    ['pdf', 'svg_zip'],
  )
  assert.equal(isInstructionExportFormat('pdf'), true)
  assert.equal(isInstructionExportFormat('svg_zip'), true)
  assert.equal(isInstructionExportFormat('svg'), false)
  assert.equal(isInstructionExportFormat('obj'), false)
  assert.equal(isInstructionExportFormat({ value: 'pdf' }), false)
  assert.equal(instructionExportFormatLabel('pdf'), 'PDF 1.7')
  assert.equal(instructionExportFormatLabel('svg_zip'), 'SVG画像 ZIP')
  assert.equal(instructionExportPhaseLabel('validating'), '入力を検証しています')
  assert.equal(instructionExportPhaseLabel('analyzing_topology'), '面構造を解析しています')
  assert.equal(instructionExportPhaseLabel('building_document'), 'ページとファイルを生成しています')
  assert.equal(instructionExportPhaseLabel('ready'), '生成が完了しました')
  assert.equal(instructionExportFormatLabel('svg_zip', 'en'), 'SVG images ZIP')
  assert.equal(
    instructionExportPhaseLabel('analyzing_topology', 'en'),
    'Analyzing face topology',
  )
})

test('preview and save responses retain the native boundary metadata', () => {
  const response: InstructionExportPreviewResponse = {
    preview: {
      export_id: '018f47d1-5ca0-75b1-a53a-c579f39f9661',
      expected_project_id: '018f47d1-5ca0-75b1-a53a-c579f39f9662',
      expected_revision: 12,
      format: 'pdf',
      profile: INSTRUCTION_EXPORT_PROFILE,
      projection_profile: INSTRUCTION_EXPORT_PROJECTION_PROFILE,
      format_summary: 'PDF 1.7・固定アイソメトリック投影',
      suggested_file_name: '鶴-折り図.pdf',
      byte_count: 2_345,
      step_count: 18,
      page_count: 6,
      caution_count: 2,
      warnings: [{
        category: 'fixed_automatic_camera',
        message_ja: '固定カメラで出力します。',
      }],
    },
  }
  const save: InstructionExportSaveResponse = { canceled: false }

  assert.equal(response.preview.step_count, 18)
  assert.equal(response.preview.page_count, 6)
  assert.equal(response.preview.caution_count, 2)
  assert.equal(response.preview.profile, 'instruction_export_v1')
  assert.equal(response.preview.projection_profile, 'orthographic_isometric_v1')
  assert.equal(save.canceled, false)
})

test('instruction export byte formatting rejects unsafe metadata and uses decimal units', () => {
  assert.equal(formatInstructionExportBytes(999), '999 B')
  assert.equal(formatInstructionExportBytes(1_500), '1.5 KB')
  assert.equal(formatInstructionExportBytes(2_500_000), '2.5 MB')
  assert.equal(formatInstructionExportBytes(-1), '不明')
  assert.equal(formatInstructionExportBytes(Number.MAX_VALUE), '不明')
  assert.equal(formatInstructionExportBytes(Number.MAX_VALUE, 'en'), 'Unknown')
})

test('instruction export errors use a closed category and never expose raw values', () => {
  const changed = createInstructionExportError('project_changed')
  assert.deepEqual(changed, {
    category: 'project_changed',
    message_ja: '生成を開始した後に編集内容が変わりました。現在の編集内容から作り直してください。',
  })
  assert.equal(instructionExportErrorMessage(changed), changed.message_ja)
  assert.equal(
    instructionExportErrorMessage(changed, 'en'),
    'The project changed after generation started. Rebuild from the current edits.',
  )

  const privateValue = 'C:\\Users\\alice\\秘密の作品.ori'
  const fallback = instructionExportErrorMessage(privateValue)
  assert.equal(fallback, '折り図書き出しを完了できませんでした。')
  assert.doesNotMatch(fallback, /alice|秘密の作品/iu)
  assert.equal(
    instructionExportErrorMessage(privateValue, 'en'),
    'Instruction export could not be completed.',
  )
  assert.equal(
    instructionExportErrorMessage({
      category: 'not_in_the_allowlist',
      message_ja: privateValue,
    }),
    fallback,
  )
})

test('instruction export warning categories select trusted Japanese and English text', () => {
  const warning = {
    category: 'discrete_step_endpoints_only',
    message_ja: 'C:\\private\\untrusted.ori2',
  } as const
  assert.equal(
    instructionExportWarningMessage(warning),
    '各手順は保存済みの終端姿勢のみを表し、手順間の連続動作は出力されません。',
  )
  assert.equal(
    instructionExportWarningMessage(warning, 'en'),
    'Each step shows only its saved endpoint pose; continuous motion between steps is not exported.',
  )
  assert.equal(
    instructionExportWarningMessage({
      category: 'unknown',
      message_ja: 'C:\\private\\untrusted.ori2',
    }, 'en'),
    'An instruction export limitation could not be identified.',
  )
})

test('instruction export error normalization fails closed for hostile objects', () => {
  const hostile = Object.create(null) as Record<string, unknown>
  Object.defineProperty(hostile, 'category', {
    get() {
      throw new Error('C:\\Users\\alice\\private.ori')
    },
  })
  assert.equal(
    instructionExportErrorMessage(hostile),
    '折り図書き出しを完了できませんでした。',
  )
})
