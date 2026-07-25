import assert from 'node:assert/strict'
import test from 'node:test'

import { formatLocalizedText } from '../src/lib/i18n.ts'
import { SVG_IMPORT_DIALOG_TEXT as TEXT } from '../src/lib/svgImportDialogText.ts'

const EXPECTED_CATALOG = [
  ['eyebrow', 'SVG 1.1 / 2 静的線図取込', 'SVG 1.1 / 2 static line import'],
  ['title', '外周・線種・縮尺を確認', 'Review boundary, line types, and scale'],
  ['close', '閉じる', 'Close'],
  [
    'description',
    '元のSVGは変更しません。直線を交点で分割し、編集可能な未保存プロジェクトとして取り込みます。',
    'The source SVG is not changed. Straight lines are split at intersections and imported as an editable, unsaved project.',
  ],
  [
    'previewLabel',
    '取り込むSVG線図のプレビュー',
    'Preview of the SVG line drawing to import',
  ],
  [
    'previewUnavailable',
    'プレビューを表示できません。',
    'The preview cannot be displayed.',
  ],
  [
    'previewTruncated',
    '表示用に一部の線だけを描画しています。',
    'Only some lines are drawn in the preview.',
  ],
  ['fileLabel', 'ファイル', 'File'],
  ['selectedFileFallback', '選択したSVGファイル', 'Selected SVG file'],
  ['segmentsLabel', '線分', 'Segments'],
  ['styleGroupsLabel', '線種候補', 'Line type candidates'],
  ['boundaryCandidatesLabel', '外周候補', 'Boundary candidates'],
  ['viewBoxLabel', 'viewBox', 'viewBox'],
  ['physicalSizeLabel', 'SVG記載の実寸', 'Physical size in SVG'],
  ['projectName', '作品名', 'Project name'],
  [
    'projectNameHelp',
    '制御文字を含まない120文字以内の名前が必要です。',
    'Enter a name of at most 120 characters without control characters.',
  ],
  ['scaleLabel', '1 SVG単位の長さ', 'Length of one SVG unit'],
  ['millimetresUnit', 'mm', 'mm'],
  [
    'scaleRequiredHelp',
    '物理単位を一意に決められないため、実寸への換算値を指定してください。',
    'The physical unit is ambiguous. Enter a conversion to the actual size.',
  ],
  [
    'scaleDetectedHelp',
    'SVGの単位とviewBoxから算出した値です。必要なら変更できます。',
    'Calculated from the SVG unit and viewBox. You can change it if needed.',
  ],
  ['boundaryTitle', '用紙外周', 'Paper boundary'],
  [
    'boundaryDescription',
    '最大の輪郭を自動採用せず、紙として使う外周を明示してください。',
    'Explicitly choose the paper boundary; the largest outline is not selected automatically.',
  ],
  ['boundaryMethod', '外周の指定方法', 'Boundary selection method'],
  ['selectPrompt', '選択してください', 'Select an option'],
  [
    'boundaryFromGroups',
    '下の線種割当で「用紙境界」を指定',
    'Assign “Paper boundary” in the line types below',
  ],
  [
    'boundaryGroupRequired',
    '少なくとも1組の線を「用紙境界」へ割り当ててください。',
    'Assign at least one line group to “Paper boundary”.',
  ],
  [
    'boundaryConflict',
    '閉じた輪郭を使う場合、線種側へ用紙境界を重ねて指定できません。',
    'When using a closed outline, a paper boundary cannot also be assigned in the line groups.',
  ],
  [
    'validatedDimensions',
    'Rust検証済みの用紙寸法: {width} × {height} mm',
    'Rust-validated paper size: {width} × {height} mm',
  ],
  [
    'validateGuidance',
    '現在の線種割当と縮尺で外周を検証し、取込後の用紙寸法を確認してください。',
    'Validate the boundary with the current line assignments and scale, then review the imported paper size.',
  ],
  ['validatingBoundary', '外周を検証中…', 'Validating boundary…'],
  ['revalidateBoundary', '外周と寸法を再検証', 'Revalidate boundary and size'],
  ['validateBoundary', '外周と寸法を検証', 'Validate boundary and size'],
  [
    'confirmBoundary',
    'Rustで検証済みの境界と寸法を、この作品の用紙外周として使用する',
    'Use the Rust-validated boundary and dimensions as this project’s paper boundary',
  ],
  ['mappingTitle', '色・線種・属性の割当', 'Assign colors, line types, and attributes'],
  [
    'mappingDescription',
    '色だけに頼らず、破線、class、レイヤー、属性を併記します。',
    'Dash patterns, classes, layers, and attributes are shown so the mapping does not rely on color alone.',
  ],
  [
    'styleGroupSummary',
    '線種候補 {index} · {elements} / {segments}',
    'Line type candidate {index} · {elements} / {segments}',
  ],
  [
    'styleLossBadge',
    '表示属性は取込後に保存しません',
    'Display attributes will not be saved after import',
  ],
  [
    'mappingLabel',
    '線種候補 {index} の割当',
    'Assignment for line type candidate {index}',
  ],
  [
    'unresolvedGroups',
    '未選択の線種候補: {groups}',
    'Unassigned line type candidates: {groups}',
  ],
  ['listSeparator', '、', ', '],
  ['cutTitle', '切断を許可', 'Allow cutting'],
  [
    'cutDescription',
    '「切断線」として割り当てた線を残すため、取込後の作品では切断を許可します。',
    'Cutting will be allowed in the imported project so lines assigned as “Cut line” can be retained.',
  ],
  ['cutConfirmation', 'この作品で切断を許可する', 'Allow cutting in this project'],
  [
    'warningsTitle',
    '取り込まれない・変更される情報',
    'Information that will not be imported or will be changed',
  ],
  [
    'warningsConfirmation',
    '上記を確認し、直線の展開図として取り込む',
    'I reviewed the above and want to import it as a straight-line crease pattern',
  ],
  ['cancel', 'キャンセル', 'Cancel'],
  ['importing', '取込中…', 'Importing…'],
  ['importAction', '取り込む', 'Import'],
  ['viewBoxCandidate', 'SVGページ矩形を生成', 'Generate SVG page rectangle'],
  ['indexedCandidate', '{kind} {index}', '{kind} {index}'],
  ['polygonCandidate', 'polygon由来の閉じた輪郭', 'Closed outline from polygon'],
  ['polylineCandidate', 'polyline由来の閉じた輪郭', 'Closed outline from polyline'],
  ['rectangleCandidate', 'rect由来の閉じた輪郭', 'Closed outline from rect'],
  ['pathCandidate', 'path由来の閉じた輪郭', 'Closed outline from path'],
  [
    'boundaryCandidateSummary',
    '{source} · {edges} · {width} × {height}単位',
    '{source} · {edges} · {width} × {height} units',
  ],
  ['notSpecified', '記載なし', 'Not specified'],
  ['originalUnit', '{value}（元: {unit}）', '{value} (source: {unit})'],
  ['unitless', '単位なし', 'unitless'],
  ['segmentCount', '{count}本', '{count} segments'],
  ['segmentCountOne', '{count}本', '{count} segment'],
  ['styleGroupCount', '{count}組', '{count} groups'],
  ['styleGroupCountOne', '{count}組', '{count} group'],
  ['candidateCount', '{count}件', '{count} candidates'],
  ['candidateCountOne', '{count}件', '{count} candidate'],
  ['elementCount', '{count}要素', '{count} elements'],
  ['elementCountOne', '{count}要素', '{count} element'],
  ['edgeCount', '{count}辺', '{count} edges'],
  ['edgeCountOne', '{count}辺', '{count} edge'],
] as const

test('SVG import dialog catalog is exact, closed, and deeply frozen', () => {
  assert.deepEqual(
    Object.keys(TEXT),
    EXPECTED_CATALOG.map(([key]) => key),
  )
  assert.deepEqual(
    Object.entries(TEXT).map(([key, text]) => [key, text.ja, text.en]),
    EXPECTED_CATALOG,
  )
  assert.equal(Object.isFrozen(TEXT), true)
  for (const text of Object.values(TEXT)) {
    assert.deepEqual(Object.keys(text), ['ja', 'en'])
    assert.equal(Object.isFrozen(text), true)
  }
})

test('SVG import dialog placeholders are closed and preserve formatting', () => {
  const placeholders = Object.fromEntries(
    Object.entries(TEXT).flatMap(([key, text]) => {
      const ja = [...text.ja.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\}/gu)]
        .map((match) => match[1])
      const en = [...text.en.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\}/gu)]
        .map((match) => match[1])
      return ja.length === 0 && en.length === 0
        ? []
        : [[key, { ja, en }]]
    }),
  )
  assert.deepEqual(placeholders, {
    validatedDimensions: {
      ja: ['width', 'height'],
      en: ['width', 'height'],
    },
    styleGroupSummary: {
      ja: ['index', 'elements', 'segments'],
      en: ['index', 'elements', 'segments'],
    },
    mappingLabel: { ja: ['index'], en: ['index'] },
    unresolvedGroups: { ja: ['groups'], en: ['groups'] },
    indexedCandidate: {
      ja: ['kind', 'index'],
      en: ['kind', 'index'],
    },
    boundaryCandidateSummary: {
      ja: ['source', 'edges', 'width', 'height'],
      en: ['source', 'edges', 'width', 'height'],
    },
    originalUnit: {
      ja: ['value', 'unit'],
      en: ['value', 'unit'],
    },
    segmentCount: { ja: ['count'], en: ['count'] },
    segmentCountOne: { ja: ['count'], en: ['count'] },
    styleGroupCount: { ja: ['count'], en: ['count'] },
    styleGroupCountOne: { ja: ['count'], en: ['count'] },
    candidateCount: { ja: ['count'], en: ['count'] },
    candidateCountOne: { ja: ['count'], en: ['count'] },
    elementCount: { ja: ['count'], en: ['count'] },
    elementCountOne: { ja: ['count'], en: ['count'] },
    edgeCount: { ja: ['count'], en: ['count'] },
    edgeCountOne: { ja: ['count'], en: ['count'] },
  })
  assert.equal(
    formatLocalizedText('ja', TEXT.validatedDimensions, {
      width: '100',
      height: '80',
    }),
    'Rust検証済みの用紙寸法: 100 × 80 mm',
  )
  assert.equal(
    formatLocalizedText('en', TEXT.styleGroupSummary, {
      index: 1,
      elements: '1 element',
      segments: '4 segments',
    }),
    'Line type candidate 1 · 1 element / 4 segments',
  )
  assert.equal(
    formatLocalizedText('en', TEXT.boundaryCandidateSummary, {
      source: 'Closed outline from polygon 1',
      edges: '4 edges',
      width: '100',
      height: '80',
    }),
    'Closed outline from polygon 1 · 4 edges · 100 × 80 units',
  )
  assert.equal(
    formatLocalizedText('ja', TEXT.originalUnit, {
      value: '100 mm',
      unit: 'px',
    }),
    '100 mm（元: px）',
  )
})
