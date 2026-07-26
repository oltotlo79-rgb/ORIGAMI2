import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
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
  assertDeeplyFrozen(CREASE_PATTERN_EXPORT_FORMATS)
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
  assert.equal(
    formatCreasePatternExportBytes(-1, 'unsupported-locale' as never),
    '不明',
  )
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
      '1件の折り手順はSVG出力に含まれません。',
      'svg',
      'en',
    ),
    '1 folding step is not included in the SVG export.',
  )
  assert.equal(
    creasePatternExportWarningMessage(
      '実寸で印刷するには、PDF viewerの印刷倍率を100%にし「用紙に合わせる」を無効にしてください。',
      'pdf',
      'en',
    ),
    'To print at full size, set the PDF viewer scale to 100% and disable “Fit to page.”',
  )
  const remainingVocabulary = [
    [
      'ORIGAMI2の頂点・辺ID、編集履歴、選択状態はSVG出力に含まれません。',
      'svg',
      'ORIGAMI2 vertex and edge IDs, edit history, and selection state are not included in the SVG export.',
    ],
    [
      '現在の3D表示姿勢とカメラ状態はPDF 1.7出力に含まれません。',
      'pdf',
      'The current 3D pose and camera state are not included in the PDF 1.7 export.',
    ],
    [
      'PDFは印刷用の視覚出力で、構造化された線種や座標原点を保持せず、ORIGAMI2へ再取込できません。',
      'pdf',
      'PDF is a visual print output. It does not retain structured line types or the coordinate origin and cannot be re-imported into ORIGAMI2.',
    ],
    [
      '折り線の意味はORIGAMI2独自のDXFレイヤー名で表し、CAD固有の標準意味ではありません。',
      'dxf',
      'Fold meanings use ORIGAMI2-specific DXF layer names and are not standard CAD semantics.',
    ],
    [
      '作品名はDXFコメントに格納されますが、CADで再保存すると失われる場合があります。',
      'dxf',
      'The work name is stored in a DXF comment but may be lost when the file is resaved by CAD software.',
    ],
    [
      '切断線を作成できるプロジェクト設定は、切断線がないためDXF（AutoCAD 2007）出力に含まれません。',
      'dxf',
      'No cut line is present, so the project setting that permits cut-line creation is not included in the DXF（AutoCAD 2007） export.',
    ],
  ] as const
  for (const [nativeWarning, format, englishMessage] of remainingVocabulary) {
    assert.equal(
      creasePatternExportWarningMessage(nativeWarning, format, 'en'),
      englishMessage,
    )
    assert.equal(
      creasePatternExportWarningMessage(nativeWarning, format, 'ja'),
      nativeWarning,
    )
  }

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
  assert.equal(
    creasePatternExportWarningMessage(
      '紙の表裏色・厚み・テクスチャはSVG出力に含まれません。',
      'svg',
      'unsupported-locale' as never,
    ),
    '紙の表裏色・厚み・テクスチャはSVG出力に含まれません。',
  )
})

test('wire DTO and format validation stay separate from localized presentation', () => {
  const contractSource = readFileSync(
    new URL('../src/lib/creaseExport.ts', import.meta.url),
    'utf8',
  )
  const catalogSource = readFileSync(
    new URL('../src/lib/creaseExportDialogText.ts', import.meta.url),
    'utf8',
  )

  assert.match(
    contractSource,
    /export \{\s*CREASE_PATTERN_EXPORT_FORMATS,[\s\S]*\} from '\.\/creaseExportDialogText\.ts'/u,
  )
  assert.doesNotMatch(contractSource, /\blocale\b/u)
  assert.doesNotMatch(contractSource, /\.toLocaleString\(/u)
  assert.doesNotMatch(
    contractSource,
    /(?:書き出しに含まれない|Unknown|folding steps are|外周|山折り)/u,
  )
  assert.match(catalogSource, /function resolveLocale\(locale: unknown\): Locale/u)
  assert.match(catalogSource, /isLocale\(locale\) \? locale : DEFAULT_LOCALE/u)
})

function assertDeeplyFrozen(value: unknown) {
  if (typeof value !== 'object' || value === null) return
  assert.equal(Object.isFrozen(value), true)
  for (const child of Object.values(value)) assertDeeplyFrozen(child)
}
