import type {
  InstructionExportErrorCategory,
  InstructionExportFormat,
  InstructionExportPhase,
  InstructionExportWarning,
} from './instructionExport.ts'
import type { LocalizedText } from './i18n.ts'

type InstructionExportPresentationTextKey =
  | 'unknownWarning'
  | 'unknownBytes'
  | 'numberLocale'
  | 'bytes'
  | 'kilobytes'
  | 'megabytes'

function localized(ja: string, en: string): LocalizedText {
  return Object.freeze({ ja, en })
}

export const INSTRUCTION_EXPORT_ERROR_TEXT: Readonly<
  Record<InstructionExportErrorCategory, LocalizedText>
> = Object.freeze({
  state_unavailable: localized(
    '折り図書き出しの状態を利用できません。アプリを再起動してください。',
    'Instruction export state is unavailable. Restart the app.',
  ),
  generation_unavailable: localized(
    'この折り図生成は利用できません。現在の編集内容から作り直してください。',
    'This instruction generation is unavailable. Rebuild it from the current edits.',
  ),
  generation_replaced: localized(
    'この折り図生成は新しい処理に置き換えられました。',
    'This instruction generation was replaced by a newer operation.',
  ),
  generation_cancelled: localized(
    '折り図の生成はキャンセルされました。',
    'Instruction generation was canceled.',
  ),
  project_changed: localized(
    '生成を開始した後に編集内容が変わりました。現在の編集内容から作り直してください。',
    'The project changed after generation started. Rebuild from the current edits.',
  ),
  timeline_empty: localized(
    '折り手順が1件もないため、折り図を書き出せません。',
    'Instructions cannot be exported because the timeline has no steps.',
  ),
  timeline_stale: localized(
    '現在の展開図より古い折り手順があります。該当する姿勢を取り直してください。',
    'Some instruction steps predate the current crease pattern. Recapture their poses.',
  ),
  source_limit_exceeded: localized(
    '折り図の元データが初版の処理上限を超えています。',
    'The instruction source exceeds the processing limits of this release.',
  ),
  topology_unsupported: localized(
    '現在の展開図は3D折り図を生成できる面構造になっていません。',
    'The current crease pattern does not have a face structure supported for 3D instructions.',
  ),
  document_input_invalid: localized(
    '折り図に含められない文字または手順情報があります。',
    'Some characters or step data cannot be included in the instructions.',
  ),
  document_limit_exceeded: localized(
    '折り図のページ数またはデータ量が初版の出力上限を超えています。',
    'The page count or data size exceeds the export limits of this release.',
  ),
  document_generation_failed: localized(
    '折り図データを生成できませんでした。',
    'Instruction data could not be generated.',
  ),
  document_contract_invalid: localized(
    '生成された折り図が対応する出力仕様と一致しません。',
    'The generated instructions do not match the supported export contract.',
  ),
  warning_acknowledgement_required: localized(
    '折り図の制約に関する確認が必要です。',
    'The instruction limitations must be acknowledged.',
  ),
  save_target_invalid: localized(
    '選択された保存先を折り図の保存先として使用できません。',
    'The selected destination cannot be used for instruction export.',
  ),
  save_failed: localized(
    '折り図ファイルを安全に保存できませんでした。保存先を変えて再試行してください。',
    'The instruction file could not be saved safely. Choose another destination and retry.',
  ),
  unexpected_failure: localized(
    '折り図書き出しを完了できませんでした。',
    'Instruction export could not be completed.',
  ),
})

export const INSTRUCTION_EXPORT_WARNING_TEXT: Readonly<
  Record<InstructionExportWarning['category'], LocalizedText>
> = Object.freeze({
  fixed_automatic_camera: localized(
    '固定自動カメラで生成され、現在のカメラや作家指定カメラは使用されません。',
    'A fixed automatic camera is used; the current camera and author-defined cameras are not used.',
  ),
  visual_effects_omitted: localized(
    'テクスチャ、照明、影、透明効果を省略し、単色の表裏色と白背景で描画します。',
    'Textures, lighting, shadows, and transparency are omitted; pages use solid front/back colors on white.',
  ),
  authored_guides_omitted: localized(
    'カメラ遷移、矢印、注目箇所、指先、つまみ、押さえ、手の移動、持ち替えは出力されません。',
    'Camera transitions, arrows, callouts, fingers, pinches, holds, hand movements, and regrips are not exported.',
  ),
  discrete_step_endpoints_only: localized(
    '各手順は保存済みの終端姿勢のみを表し、手順間の連続動作は出力されません。',
    'Each step shows only its saved endpoint pose; continuous motion between steps is not exported.',
  ),
})

export const INSTRUCTION_EXPORT_FORMAT_LABEL_TEXT: Readonly<
  Record<InstructionExportFormat, LocalizedText>
> = Object.freeze({
  pdf: localized('PDF 1.7', 'PDF 1.7'),
  svg_zip: localized('SVG画像 ZIP', 'SVG images ZIP'),
})

export const INSTRUCTION_EXPORT_PHASE_TEXT: Readonly<
  Record<InstructionExportPhase, LocalizedText>
> = Object.freeze({
  validating: localized('入力を検証しています', 'Validating input'),
  analyzing_topology: localized(
    '面構造を解析しています',
    'Analyzing face topology',
  ),
  building_document: localized(
    'ページとファイルを生成しています',
    'Generating pages and files',
  ),
  ready: localized('生成が完了しました', 'Generation complete'),
})

export const INSTRUCTION_EXPORT_PRESENTATION_TEXT: Readonly<
  Record<InstructionExportPresentationTextKey, LocalizedText>
> = Object.freeze({
  unknownWarning: localized(
    '折り図の制約を識別できません。',
    'An instruction export limitation could not be identified.',
  ),
  unknownBytes: localized('不明', 'Unknown'),
  numberLocale: localized('ja-JP', 'en-US'),
  bytes: localized('{value} B', '{value} B'),
  kilobytes: localized('{value} KB', '{value} KB'),
  megabytes: localized('{value} MB', '{value} MB'),
})
