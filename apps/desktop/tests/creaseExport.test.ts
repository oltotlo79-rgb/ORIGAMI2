import assert from 'node:assert/strict'
import test from 'node:test'

import {
  CREASE_PATTERN_EXPORT_FORMATS,
  creasePatternExportAssignmentRows,
  creasePatternExportFormatLabel,
  creasePatternExportWarningMessage,
  formatCreasePatternExportBytes,
  isCreasePatternExportFormat,
} from '../src/lib/creaseExport.ts'

test('export formats are a closed FOLD/SVG/PDF/DXF set with stable labels', () => {
  assert.deepEqual(
    CREASE_PATTERN_EXPORT_FORMATS.map(({ value }) => value),
    ['fold', 'svg', 'pdf', 'dxf'],
  )
  assert.equal(isCreasePatternExportFormat('fold'), true)
  assert.equal(isCreasePatternExportFormat('svg'), true)
  assert.equal(isCreasePatternExportFormat('pdf'), true)
  assert.equal(isCreasePatternExportFormat('dxf'), true)
  assert.equal(isCreasePatternExportFormat('obj'), false)
  assert.equal(isCreasePatternExportFormat({ value: 'fold' }), false)
  assert.equal(creasePatternExportFormatLabel('fold'), 'FOLD 1.2')
  assert.equal(creasePatternExportFormatLabel('svg'), 'SVG')
  assert.equal(creasePatternExportFormatLabel('pdf'), 'PDF 1.7')
  assert.equal(creasePatternExportFormatLabel('dxf'), 'DXF（AutoCAD 2007）')
})

test('assignment rows preserve every supported edge kind in display order', () => {
  assert.deepEqual(
    creasePatternExportAssignmentRows({
      boundary: 4,
      mountain: 5,
      valley: 6,
      auxiliary: 7,
      cut: 8,
    }),
    [
      { key: 'boundary', label: '外周', count: 4 },
      { key: 'mountain', label: '山折り', count: 5 },
      { key: 'valley', label: '谷折り', count: 6 },
      { key: 'auxiliary', label: '補助線', count: 7 },
      { key: 'cut', label: '切断線', count: 8 },
    ],
  )
})

test('byte formatting rejects unsafe metadata and uses decimal units', () => {
  assert.equal(formatCreasePatternExportBytes(999), '999 B')
  assert.equal(formatCreasePatternExportBytes(1_500), '1.5 KB')
  assert.equal(formatCreasePatternExportBytes(2_500_000), '2.5 MB')
  assert.equal(formatCreasePatternExportBytes(-1), '不明')
  assert.equal(formatCreasePatternExportBytes(Number.MAX_VALUE), '不明')
  assert.equal(formatCreasePatternExportBytes(-1, 'en'), 'Unknown')
})

test('native export warnings are localized from a fixed vocabulary without leaking unknown text', () => {
  assert.equal(
    creasePatternExportWarningMessage(
      '紙の表裏色・厚み・テクスチャはFOLD 1.2出力に含まれません。',
      'fold',
      'en',
    ),
    'The front and back paper colors, thickness, and texture are not included in the FOLD 1.2 export.',
  )
  assert.equal(
    creasePatternExportWarningMessage(
      '12件の折り手順はPDF 1.7出力に含まれません。',
      'pdf',
      'en',
    ),
    '12 folding steps are not included in the PDF 1.7 export.',
  )
  assert.equal(
    creasePatternExportWarningMessage(
      '実寸で印刷するには、PDF viewerの印刷倍率を100%にし「用紙に合わせる」を無効にしてください。',
      'pdf',
      'en',
    ),
    'To print at full size, set the PDF viewer scale to 100% and disable “Fit to page.”',
  )

  const privateWarning = String.raw`C:\Users\alice\private-project.ori2`
  const fallback = creasePatternExportWarningMessage(
    privateWarning,
    'dxf',
    'en',
  )
  assert.equal(
    fallback,
    'Some project information is not included in this export.',
  )
  assert.doesNotMatch(fallback, /alice|private-project|[ぁ-んァ-ン一-龯]/u)
  const japaneseFallback = creasePatternExportWarningMessage(
    privateWarning,
    'dxf',
    'ja',
  )
  assert.equal(
    japaneseFallback,
    '書き出しに含まれないプロジェクト情報があります。',
  )
  assert.doesNotMatch(japaneseFallback, /alice|private-project/u)
})
