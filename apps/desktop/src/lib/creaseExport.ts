import type { Locale } from './i18n.ts'

export type CreasePatternExportFormat = 'fold' | 'svg' | 'pdf' | 'dxf'

export type CreasePatternExportAssignmentCounts = Readonly<{
  boundary: number
  mountain: number
  valley: number
  auxiliary: number
  cut: number
}>

export type CreasePatternExportPreview = Readonly<{
  export_id: string
  expected_project_id: string
  expected_revision: number
  format: CreasePatternExportFormat
  format_summary: string
  suggested_file_name: string
  byte_count: number
  vertex_count: number
  edge_count: number
  assignment_counts: CreasePatternExportAssignmentCounts
  has_cuts: boolean
  warnings: readonly string[]
}>

export type CreasePatternExportSaveResponse = Readonly<{
  canceled: boolean
}>

export const CREASE_PATTERN_EXPORT_FORMATS:
ReadonlyArray<Readonly<{ value: CreasePatternExportFormat; label: string; detail: string }>> =
  Object.freeze([
    {
      value: 'fold',
      label: 'FOLD 1.2',
      detail: '他の折り紙ソフトと交換しやすいJSON形式',
    },
    {
      value: 'svg',
      label: 'SVG',
      detail: '印刷・作図ソフトで扱いやすい静的な線図',
    },
    {
      value: 'pdf',
      label: 'PDF 1.7',
      detail: '実寸1:1・四辺10 mm余白の白黒ベクター印刷',
    },
    {
      value: 'dxf',
      label: 'DXF',
      detail: 'AutoCAD 2007・mm・5意味レイヤーのCAD交換',
    },
  ])

export function isCreasePatternExportFormat(
  value: unknown,
): value is CreasePatternExportFormat {
  return value === 'fold' || value === 'svg' || value === 'pdf' || value === 'dxf'
}

export function creasePatternExportFormatLabel(format: CreasePatternExportFormat) {
  switch (format) {
    case 'fold':
      return 'FOLD 1.2'
    case 'svg':
      return 'SVG'
    case 'pdf':
      return 'PDF 1.7'
    case 'dxf':
      return 'DXF（AutoCAD 2007）'
  }
}

export function creasePatternExportAssignmentRows(
  counts: CreasePatternExportAssignmentCounts,
) {
  return [
    { key: 'boundary', label: '外周', count: counts.boundary },
    { key: 'mountain', label: '山折り', count: counts.mountain },
    { key: 'valley', label: '谷折り', count: counts.valley },
    { key: 'auxiliary', label: '補助線', count: counts.auxiliary },
    { key: 'cut', label: '切断線', count: counts.cut },
  ] as const
}

export function formatCreasePatternExportBytes(
  bytes: number,
  locale: Locale = 'ja',
) {
  if (!Number.isSafeInteger(bytes) || bytes < 0) {
    return locale === 'en' ? 'Unknown' : '不明'
  }
  if (bytes < 1_000) {
    return `${bytes.toLocaleString(locale === 'en' ? 'en-US' : 'ja-JP')} B`
  }
  if (bytes < 1_000_000) return `${(bytes / 1_000).toFixed(1)} KB`
  return `${(bytes / 1_000_000).toFixed(1)} MB`
}

export function creasePatternExportWarningMessage(
  warning: unknown,
  format: CreasePatternExportFormat,
  locale: Locale = 'ja',
) {
  const category = classifyCreasePatternExportWarning(warning)
  if (locale === 'ja') {
    return category === null
      ? '書き出しに含まれないプロジェクト情報があります。'
      : warning as string
  }

  const label = creasePatternExportFormatLabel(format)
  switch (category?.kind) {
    case 'paper_appearance':
      return `The front and back paper colors, thickness, and texture are not included in the ${label} export.`
    case 'editor_state':
      return `ORIGAMI2 vertex and edge IDs, edit history, and selection state are not included in the ${label} export.`
    case 'pose_camera':
      return `The current 3D pose and camera state are not included in the ${label} export.`
    case 'pdf_structure':
      return 'PDF is a visual print output. It does not retain structured line types or the coordinate origin and cannot be re-imported into ORIGAMI2.'
    case 'pdf_print_scale':
      return 'To print at full size, set the PDF viewer scale to 100% and disable “Fit to page.”'
    case 'dxf_layers':
      return 'Fold meanings use ORIGAMI2-specific DXF layer names and are not standard CAD semantics.'
    case 'dxf_name':
      return 'The work name is stored in a DXF comment but may be lost when the file is resaved by CAD software.'
    case 'instruction_steps': {
      const unit = category.count === 1 ? 'folding step is' : 'folding steps are'
      return `${category.count.toLocaleString('en-US')} ${unit} not included in the ${label} export.`
    }
    case 'cut_permission':
      return `No cut line is present, so the project setting that permits cut-line creation is not included in the ${label} export.`
    default:
      return 'Some project information is not included in this export.'
  }
}

type CreasePatternExportWarningCategory =
  | Readonly<{ kind: 'paper_appearance' }>
  | Readonly<{ kind: 'editor_state' }>
  | Readonly<{ kind: 'pose_camera' }>
  | Readonly<{ kind: 'pdf_structure' }>
  | Readonly<{ kind: 'pdf_print_scale' }>
  | Readonly<{ kind: 'dxf_layers' }>
  | Readonly<{ kind: 'dxf_name' }>
  | Readonly<{ kind: 'instruction_steps'; count: number }>
  | Readonly<{ kind: 'cut_permission' }>

const EXPORT_LABEL_PATTERN = '(?:FOLD 1\\.2|SVG|PDF 1\\.7|DXF(?:（AutoCAD 2007）)?)'

function classifyCreasePatternExportWarning(
  warning: unknown,
): CreasePatternExportWarningCategory | null {
  if (typeof warning !== 'string') return null
  if (new RegExp(
    `^紙の表裏色・厚み・テクスチャは${EXPORT_LABEL_PATTERN}出力に含まれません。$`,
    'u',
  ).test(warning)) {
    return { kind: 'paper_appearance' }
  }
  if (new RegExp(
    `^ORIGAMI2の頂点・辺ID、編集履歴、選択状態は${EXPORT_LABEL_PATTERN}出力に含まれません。$`,
    'u',
  ).test(warning)) {
    return { kind: 'editor_state' }
  }
  if (new RegExp(
    `^現在の3D表示姿勢とカメラ状態は${EXPORT_LABEL_PATTERN}出力に含まれません。$`,
    'u',
  ).test(warning)) {
    return { kind: 'pose_camera' }
  }
  if (
    warning
    === 'PDFは印刷用の視覚出力で、構造化された線種や座標原点を保持せず、ORIGAMI2へ再取込できません。'
  ) {
    return { kind: 'pdf_structure' }
  }
  if (
    warning
    === '実寸で印刷するには、PDF viewerの印刷倍率を100%にし「用紙に合わせる」を無効にしてください。'
  ) {
    return { kind: 'pdf_print_scale' }
  }
  if (
    warning
    === '折り線の意味はORIGAMI2独自のDXFレイヤー名で表し、CAD固有の標準意味ではありません。'
  ) {
    return { kind: 'dxf_layers' }
  }
  if (
    warning
    === '作品名はDXFコメントに格納されますが、CADで再保存すると失われる場合があります。'
  ) {
    return { kind: 'dxf_name' }
  }
  const steps = new RegExp(
    `^([0-9]{1,20})件の折り手順は${EXPORT_LABEL_PATTERN}出力に含まれません。$`,
    'u',
  ).exec(warning)
  if (steps) {
    const count = Number(steps[1])
    if (Number.isSafeInteger(count)) {
      return { kind: 'instruction_steps', count }
    }
  }
  if (new RegExp(
    `^切断線を作成できるプロジェクト設定は、切断線がないため${EXPORT_LABEL_PATTERN}出力に含まれません。$`,
    'u',
  ).test(warning)) {
    return { kind: 'cut_permission' }
  }
  return null
}
