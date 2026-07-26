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

export function isCreasePatternExportFormat(
  value: unknown,
): value is CreasePatternExportFormat {
  return value === 'fold' || value === 'svg' || value === 'pdf' || value === 'dxf'
}

// Keep the established presentation exports available to existing callers
// while their UI-sensitive implementation lives in the dedicated catalog.
export {
  CREASE_PATTERN_EXPORT_FORMATS,
  creasePatternExportAssignmentRows,
  creasePatternExportFormatLabel,
  creasePatternExportWarningMessage,
  formatCreasePatternExportBytes,
} from './creaseExportDialogText.ts'
