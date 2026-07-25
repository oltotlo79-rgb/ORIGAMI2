import type { LocalizedText } from './i18n.ts'

export type EffectiveCutDiagnosticPanelText = Readonly<Record<
  | 'ariaLabel'
  | 'title'
  | 'explanation'
  | 'loading'
  | 'reloadCandidates'
  | 'unavailable'
  | 'reload'
  | 'candidate'
  | 'faceCount'
  | 'removalClosure'
  | 'dependencies'
  | 'running'
  | 'diagnoseSelection'
  | 'cancel'
  | 'sourceFlatPairs'
  | 'indeterminate'
  | 'multiHingeCorridorUnproved',
  LocalizedText
>>

const text = (ja: string, en: string): LocalizedText =>
  Object.freeze({ ja, en })

export const EFFECTIVE_CUT_DIAGNOSTIC_PANEL_TEXT =
  Object.freeze({
    ariaLabel: text('有効カット診断', 'Effective-cut diagnostic'),
    title: text(
      '有効カット診断（読み取り専用）',
      'Effective-cut diagnostic (read-only)',
    ),
    explanation: text(
      '候補の面積はその成分単体です。依存成分は閉包数として別に表示します。',
      'Area is for the candidate component alone; dependent components are reported as closure counts.',
    ),
    loading: text('候補を取得中…', 'Loading candidates…'),
    reloadCandidates: text('候補を再取得', 'Reload candidates'),
    unavailable: text(
      '現在の編集内容では診断できません。',
      'Diagnostics are unavailable for the current edit.',
    ),
    reload: text('再取得', 'Reload'),
    candidate: text('候補 {index}', 'Candidate {index}'),
    faceCount: text('{count} 面', '{count} faces'),
    removalClosure: text('除去範囲', 'removal closure'),
    dependencies: text(
      ' (+{count} 依存成分)',
      ' (+{count} dependencies)',
    ),
    running: text('診断中…', 'Running…'),
    diagnoseSelection: text('選択を診断', 'Diagnose selection'),
    cancel: text('キャンセル', 'Cancel'),
    sourceFlatPairs: text('平面ペア', 'Source-flat pairs'),
    indeterminate: text('未確定', 'indeterminate'),
    multiHingeCorridorUnproved: text(
      '複数ヒンジ経路未証明',
      'multi-hinge corridor unproved',
    ),
  }) satisfies EffectiveCutDiagnosticPanelText
