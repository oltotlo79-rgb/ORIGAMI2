import {
  formatLocalizedText,
  selectLocalizedText,
  type LocalizedText,
} from './i18n.ts'
import type {
  FoldAssignmentCode,
  FoldBoundaryCandidateSource,
  FoldImportTarget,
} from './foldImport.ts'

export type FoldImportWarningCategory =
  | 'missing_spec'
  | 'missing_assignments'
  | 'boundary_selection'
  | 'unit_needs_scale'
  | 'ignored_metadata'
  | 'invalid_title'
  | 'flat_crease'
  | 'unassigned'
  | 'face_join'

export type FoldImportPresentationText = Readonly<{
  assignmentLabels: Readonly<
    Record<FoldAssignmentCode | 'B', LocalizedText>
  >
  targetLabels: Readonly<Record<FoldImportTarget, LocalizedText>>
  warningMessages: Readonly<
    Record<FoldImportWarningCategory | 'unknown', LocalizedText>
  >
  previewFileNameFallback: LocalizedText
  suggestedNameFallback: LocalizedText
  boundaryCandidateLabels: Readonly<
    Record<FoldBoundaryCandidateSource, LocalizedText>
  >
  numberLocale: LocalizedText
}>

function localized(ja: string, en: string): LocalizedText {
  return Object.freeze({ ja, en })
}

const ASSIGNMENT_LABELS = Object.freeze({
  B: localized('B · 用紙境界', 'B · Paper boundary'),
  M: localized('M · 山折り', 'M · Mountain fold'),
  V: localized('V · 谷折り', 'V · Valley fold'),
  F: localized('F · 平らな折り筋', 'F · Flat crease'),
  U: localized('U · 未割当', 'U · Unassigned'),
  C: localized('C · 切断・スリット', 'C · Cut or slit'),
  J: localized('J · 面の結合', 'J · Face join'),
}) satisfies Readonly<
  Record<FoldAssignmentCode | 'B', LocalizedText>
>

const TARGET_LABELS = Object.freeze({
  mountain: localized('山折り', 'Mountain fold'),
  valley: localized('谷折り', 'Valley fold'),
  auxiliary: localized('補助線', 'Auxiliary line'),
  cut: localized('切断線', 'Cut line'),
  ignore: localized('取り込まない', 'Do not import'),
}) satisfies Readonly<Record<FoldImportTarget, LocalizedText>>

const WARNING_MESSAGES = Object.freeze({
  missing_spec: localized(
    'FOLD仕様バージョンの記載がありません。対応範囲として慎重に解釈します。',
    'The FOLD specification version is missing, so the file will be interpreted conservatively within the supported range.',
  ),
  missing_assignments: localized(
    '辺の割当情報（edges_assignment）がないため、折り線種を確認・指定してください。',
    'The optional edges_assignment array is missing. Review the paper boundary and explicitly map every remaining unassigned line.',
  ),
  boundary_selection: localized(
    '外周を一意に確定できないため、取り込む用紙外周を選択してください。',
    'The source assignments do not establish one valid paper boundary. Select the intended validated outer-boundary candidate.',
  ),
  unit_needs_scale: localized(
    '実寸へ換算できる単位情報がないため、1単位あたりのmm値を指定してください。',
    'The file has no unit information that can be converted to physical size. Enter the millimetres per FOLD unit.',
  ),
  ignored_metadata: localized(
    '取り込まないFOLD情報: {metadata}。',
    'Some FOLD metadata will not be imported.{metadata}',
  ),
  invalid_title: localized(
    'FOLD内のタイトルは作品名の条件に合わないため、既定の作品名を使用します。',
    'The title in the FOLD file does not meet the work-name requirements, so the default name will be used.',
  ),
  flat_crease: localized(
    'F（平らな折り筋）は同じ意味の線種がないため、補助線または除外へ変換します。',
    'F (flat crease) has no equivalent line type and must be converted to an auxiliary line or excluded.',
  ),
  unassigned: localized(
    'U（未割当）は山折り・谷折り・補助線・除外のいずれかを選ぶ必要があります。',
    'U (unassigned) must be mapped to a mountain fold, valley fold, auxiliary line, or exclusion.',
  ),
  face_join: localized(
    'J（面の結合）は同じ意味の線種がないため、補助線または除外へ変換します。',
    'J (face join) has no equivalent line type and must be converted to an auxiliary line or excluded.',
  ),
  unknown: localized(
    '取り込まれないFOLD情報があります。',
    'Some FOLD information will not be imported.',
  ),
}) satisfies Readonly<
  Record<FoldImportWarningCategory | 'unknown', LocalizedText>
>

const BOUNDARY_CANDIDATE_LABELS = Object.freeze({
  assigned_boundary: localized(
    '元のB線による外周（{count}辺）',
    'Boundary from source B lines ({count} edges)',
  ),
  inferred_outer_face: localized(
    '検証済み外周候補 {number}（{count}辺）',
    'Validated boundary candidate {number} ({count} edges)',
  ),
}) satisfies Readonly<
  Record<FoldBoundaryCandidateSource, LocalizedText>
>

export const FOLD_IMPORT_PRESENTATION_TEXT = Object.freeze({
  assignmentLabels: ASSIGNMENT_LABELS,
  targetLabels: TARGET_LABELS,
  warningMessages: WARNING_MESSAGES,
  previewFileNameFallback: localized(
    '選択したFOLDファイル',
    'Selected FOLD file',
  ),
  suggestedNameFallback: localized('FOLDインポート', 'FOLD import'),
  boundaryCandidateLabels: BOUNDARY_CANDIDATE_LABELS,
  numberLocale: localized('ja-JP', 'en-US'),
}) satisfies FoldImportPresentationText

export function formatFoldImportWarningPresentation(
  category: FoldImportWarningCategory | null,
  ignoredMetadata: string | null,
  locale: unknown,
) {
  if (category === null) {
    return selectLocalizedText(
      locale,
      FOLD_IMPORT_PRESENTATION_TEXT.warningMessages.unknown,
    )
  }
  if (category !== 'ignored_metadata') {
    return selectLocalizedText(
      locale,
      FOLD_IMPORT_PRESENTATION_TEXT.warningMessages[category],
    )
  }
  if (ignoredMetadata === null) {
    return selectLocalizedText(
      locale,
      FOLD_IMPORT_PRESENTATION_TEXT.warningMessages.unknown,
    )
  }
  const metadata = selectLocalizedText(
    locale,
    Object.freeze({ ja: ignoredMetadata, en: '' }),
  )
  return formatLocalizedText(
    locale,
    FOLD_IMPORT_PRESENTATION_TEXT.warningMessages.ignored_metadata,
    { metadata },
  )
}

export function formatFoldImportBoundaryCandidatePresentation(
  source: FoldBoundaryCandidateSource,
  candidateId: number,
  edgeCount: number,
  locale: unknown,
) {
  const numberLocale = selectLocalizedText(
    locale,
    FOLD_IMPORT_PRESENTATION_TEXT.numberLocale,
  )
  return formatLocalizedText(
    locale,
    FOLD_IMPORT_PRESENTATION_TEXT.boundaryCandidateLabels[source],
    {
      number: (candidateId + 1).toLocaleString(numberLocale),
      count: edgeCount.toLocaleString(numberLocale),
    },
  )
}
