export type CreasePatternExportFormat = 'fold' | 'svg'

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
  ])

export function isCreasePatternExportFormat(
  value: unknown,
): value is CreasePatternExportFormat {
  return value === 'fold' || value === 'svg'
}

export function creasePatternExportFormatLabel(format: CreasePatternExportFormat) {
  return format === 'fold' ? 'FOLD 1.2' : 'SVG'
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

export function formatCreasePatternExportBytes(bytes: number) {
  if (!Number.isSafeInteger(bytes) || bytes < 0) return '不明'
  if (bytes < 1_000) return `${bytes.toLocaleString('ja-JP')} B`
  if (bytes < 1_000_000) return `${(bytes / 1_000).toFixed(1)} KB`
  return `${(bytes / 1_000_000).toFixed(1)} MB`
}
