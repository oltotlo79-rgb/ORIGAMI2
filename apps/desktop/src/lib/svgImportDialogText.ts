import {
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
  type LocalizedText,
} from './i18n.ts'

function localized(ja: string, en: string): LocalizedText {
  return Object.freeze({ ja, en })
}

export const SVG_IMPORT_DIALOG_TEXT: Readonly<Record<
  | 'eyebrow' | 'title' | 'close' | 'description' | 'previewLabel'
  | 'previewUnavailable' | 'previewTruncated' | 'fileLabel'
  | 'selectedFileFallback' | 'segmentsLabel' | 'styleGroupsLabel'
  | 'boundaryCandidatesLabel' | 'viewBoxLabel' | 'physicalSizeLabel'
  | 'projectName' | 'projectNameHelp' | 'scaleLabel' | 'millimetresUnit'
  | 'scaleRequiredHelp' | 'scaleDetectedHelp' | 'boundaryTitle'
  | 'boundaryDescription' | 'boundaryMethod' | 'selectPrompt'
  | 'boundaryFromGroups' | 'boundaryGroupRequired' | 'boundaryConflict'
  | 'validatedDimensions' | 'validateGuidance' | 'validatingBoundary'
  | 'revalidateBoundary' | 'validateBoundary' | 'confirmBoundary'
  | 'mappingTitle' | 'mappingDescription' | 'styleGroupSummary'
  | 'styleLossBadge' | 'mappingLabel' | 'unresolvedGroups'
  | 'listSeparator' | 'cutTitle' | 'cutDescription' | 'cutConfirmation'
  | 'warningsTitle' | 'warningsConfirmation' | 'cancel' | 'importing'
  | 'importAction' | 'viewBoxCandidate' | 'indexedCandidate'
  | 'polygonCandidate' | 'polylineCandidate' | 'rectangleCandidate'
  | 'pathCandidate' | 'boundaryCandidateSummary' | 'notSpecified'
  | 'originalUnit' | 'unitless' | 'segmentCount' | 'segmentCountOne'
  | 'styleGroupCount' | 'styleGroupCountOne' | 'candidateCount'
  | 'candidateCountOne' | 'elementCount' | 'elementCountOne'
  | 'edgeCount' | 'edgeCountOne',
  LocalizedText
>> = Object.freeze({
  eyebrow: localized(
    'SVG 1.1 / 2 静的線図取込',
    'SVG 1.1 / 2 static line import',
  ),
  title: localized(
    '外周・線種・縮尺を確認',
    'Review boundary, line types, and scale',
  ),
  close: localized('閉じる', 'Close'),
  description: localized(
    '元のSVGは変更しません。直線を交点で分割し、編集可能な未保存プロジェクトとして取り込みます。',
    'The source SVG is not changed. Straight lines are split at intersections and imported as an editable, unsaved project.',
  ),
  previewLabel: localized(
    '取り込むSVG線図のプレビュー',
    'Preview of the SVG line drawing to import',
  ),
  previewUnavailable: localized(
    'プレビューを表示できません。',
    'The preview cannot be displayed.',
  ),
  previewTruncated: localized(
    '表示用に一部の線だけを描画しています。',
    'Only some lines are drawn in the preview.',
  ),
  fileLabel: localized('ファイル', 'File'),
  selectedFileFallback: localized(
    '選択したSVGファイル',
    'Selected SVG file',
  ),
  segmentsLabel: localized('線分', 'Segments'),
  styleGroupsLabel: localized('線種候補', 'Line type candidates'),
  boundaryCandidatesLabel: localized('外周候補', 'Boundary candidates'),
  viewBoxLabel: localized('viewBox', 'viewBox'),
  physicalSizeLabel: localized('SVG記載の実寸', 'Physical size in SVG'),
  projectName: localized('作品名', 'Project name'),
  projectNameHelp: localized(
    '制御文字を含まない120文字以内の名前が必要です。',
    'Enter a name of at most 120 characters without control characters.',
  ),
  scaleLabel: localized('1 SVG単位の長さ', 'Length of one SVG unit'),
  millimetresUnit: localized('mm', 'mm'),
  scaleRequiredHelp: localized(
    '物理単位を一意に決められないため、実寸への換算値を指定してください。',
    'The physical unit is ambiguous. Enter a conversion to the actual size.',
  ),
  scaleDetectedHelp: localized(
    'SVGの単位とviewBoxから算出した値です。必要なら変更できます。',
    'Calculated from the SVG unit and viewBox. You can change it if needed.',
  ),
  boundaryTitle: localized('用紙外周', 'Paper boundary'),
  boundaryDescription: localized(
    '最大の輪郭を自動採用せず、紙として使う外周を明示してください。',
    'Explicitly choose the paper boundary; the largest outline is not selected automatically.',
  ),
  boundaryMethod: localized('外周の指定方法', 'Boundary selection method'),
  selectPrompt: localized('選択してください', 'Select an option'),
  boundaryFromGroups: localized(
    '下の線種割当で「用紙境界」を指定',
    'Assign “Paper boundary” in the line types below',
  ),
  boundaryGroupRequired: localized(
    '少なくとも1組の線を「用紙境界」へ割り当ててください。',
    'Assign at least one line group to “Paper boundary”.',
  ),
  boundaryConflict: localized(
    '閉じた輪郭を使う場合、線種側へ用紙境界を重ねて指定できません。',
    'When using a closed outline, a paper boundary cannot also be assigned in the line groups.',
  ),
  validatedDimensions: localized(
    'Rust検証済みの用紙寸法: {width} × {height} mm',
    'Rust-validated paper size: {width} × {height} mm',
  ),
  validateGuidance: localized(
    '現在の線種割当と縮尺で外周を検証し、取込後の用紙寸法を確認してください。',
    'Validate the boundary with the current line assignments and scale, then review the imported paper size.',
  ),
  validatingBoundary: localized('外周を検証中…', 'Validating boundary…'),
  revalidateBoundary: localized(
    '外周と寸法を再検証',
    'Revalidate boundary and size',
  ),
  validateBoundary: localized('外周と寸法を検証', 'Validate boundary and size'),
  confirmBoundary: localized(
    'Rustで検証済みの境界と寸法を、この作品の用紙外周として使用する',
    'Use the Rust-validated boundary and dimensions as this project’s paper boundary',
  ),
  mappingTitle: localized(
    '色・線種・属性の割当',
    'Assign colors, line types, and attributes',
  ),
  mappingDescription: localized(
    '色だけに頼らず、破線、class、レイヤー、属性を併記します。',
    'Dash patterns, classes, layers, and attributes are shown so the mapping does not rely on color alone.',
  ),
  styleGroupSummary: localized(
    '線種候補 {index} · {elements} / {segments}',
    'Line type candidate {index} · {elements} / {segments}',
  ),
  styleLossBadge: localized(
    '表示属性は取込後に保存しません',
    'Display attributes will not be saved after import',
  ),
  mappingLabel: localized(
    '線種候補 {index} の割当',
    'Assignment for line type candidate {index}',
  ),
  unresolvedGroups: localized(
    '未選択の線種候補: {groups}',
    'Unassigned line type candidates: {groups}',
  ),
  listSeparator: localized('、', ', '),
  cutTitle: localized('切断を許可', 'Allow cutting'),
  cutDescription: localized(
    '「切断線」として割り当てた線を残すため、取込後の作品では切断を許可します。',
    'Cutting will be allowed in the imported project so lines assigned as “Cut line” can be retained.',
  ),
  cutConfirmation: localized(
    'この作品で切断を許可する',
    'Allow cutting in this project',
  ),
  warningsTitle: localized(
    '取り込まれない・変更される情報',
    'Information that will not be imported or will be changed',
  ),
  warningsConfirmation: localized(
    '上記を確認し、直線の展開図として取り込む',
    'I reviewed the above and want to import it as a straight-line crease pattern',
  ),
  cancel: localized('キャンセル', 'Cancel'),
  importing: localized('取込中…', 'Importing…'),
  importAction: localized('取り込む', 'Import'),
  viewBoxCandidate: localized(
    'SVGページ矩形を生成',
    'Generate SVG page rectangle',
  ),
  indexedCandidate: localized('{kind} {index}', '{kind} {index}'),
  polygonCandidate: localized(
    'polygon由来の閉じた輪郭',
    'Closed outline from polygon',
  ),
  polylineCandidate: localized(
    'polyline由来の閉じた輪郭',
    'Closed outline from polyline',
  ),
  rectangleCandidate: localized(
    'rect由来の閉じた輪郭',
    'Closed outline from rect',
  ),
  pathCandidate: localized(
    'path由来の閉じた輪郭',
    'Closed outline from path',
  ),
  boundaryCandidateSummary: localized(
    '{source} · {edges} · {width} × {height}単位',
    '{source} · {edges} · {width} × {height} units',
  ),
  notSpecified: localized('記載なし', 'Not specified'),
  originalUnit: localized(
    '{value}（元: {unit}）',
    '{value} (source: {unit})',
  ),
  unitless: localized('単位なし', 'unitless'),
  segmentCount: localized('{count}本', '{count} segments'),
  segmentCountOne: localized('{count}本', '{count} segment'),
  styleGroupCount: localized('{count}組', '{count} groups'),
  styleGroupCountOne: localized('{count}組', '{count} group'),
  candidateCount: localized('{count}件', '{count} candidates'),
  candidateCountOne: localized('{count}件', '{count} candidate'),
  elementCount: localized('{count}要素', '{count} elements'),
  elementCountOne: localized('{count}要素', '{count} element'),
  edgeCount: localized('{count}辺', '{count} edges'),
  edgeCountOne: localized('{count}辺', '{count} edge'),
})

export type SvgImportCountKind =
  | 'segment'
  | 'styleGroup'
  | 'candidate'
  | 'element'
  | 'edge'

type SvgImportCountText = Readonly<{
  one: LocalizedText
  other: LocalizedText
}>

const SVG_IMPORT_NUMBER_LOCALES = Object.freeze({
  ja: 'ja-JP',
  en: 'en-US',
}) satisfies Readonly<Record<Locale, string>>

const SVG_IMPORT_COUNT_TEXT = Object.freeze({
  segment: Object.freeze({
    one: SVG_IMPORT_DIALOG_TEXT.segmentCountOne,
    other: SVG_IMPORT_DIALOG_TEXT.segmentCount,
  }),
  styleGroup: Object.freeze({
    one: SVG_IMPORT_DIALOG_TEXT.styleGroupCountOne,
    other: SVG_IMPORT_DIALOG_TEXT.styleGroupCount,
  }),
  candidate: Object.freeze({
    one: SVG_IMPORT_DIALOG_TEXT.candidateCountOne,
    other: SVG_IMPORT_DIALOG_TEXT.candidateCount,
  }),
  element: Object.freeze({
    one: SVG_IMPORT_DIALOG_TEXT.elementCountOne,
    other: SVG_IMPORT_DIALOG_TEXT.elementCount,
  }),
  edge: Object.freeze({
    one: SVG_IMPORT_DIALOG_TEXT.edgeCountOne,
    other: SVG_IMPORT_DIALOG_TEXT.edgeCount,
  }),
}) satisfies Readonly<Record<SvgImportCountKind, SvgImportCountText>>

export function formatSvgImportNumber(value: number, locale: Locale) {
  return Number.isFinite(value)
    ? value.toLocaleString(
        SVG_IMPORT_NUMBER_LOCALES[locale],
        { maximumSignificantDigits: 12 },
      )
    : '?'
}

export function formatSvgImportSourceFileLabel(
  value: string,
  locale: Locale,
) {
  return value === SVG_IMPORT_DIALOG_TEXT.selectedFileFallback.ja
    ? selectLocalizedText(locale, SVG_IMPORT_DIALOG_TEXT.selectedFileFallback)
    : value
}

export function formatSvgImportCount(
  count: number,
  kind: SvgImportCountKind,
  locale: Locale,
) {
  const copy = SVG_IMPORT_COUNT_TEXT[kind]
  return formatLocalizedText(locale, count === 1 ? copy.one : copy.other, {
    count: count.toLocaleString(SVG_IMPORT_NUMBER_LOCALES[locale]),
  })
}
