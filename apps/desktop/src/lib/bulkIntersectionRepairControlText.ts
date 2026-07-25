import type { LocalizedText } from './i18n.ts'

export type BulkIntersectionRepairControlText = Readonly<Record<
  'repairing' | 'repairAll' | 'confirmation',
  LocalizedText
>>

const text = (ja: string, en: string): LocalizedText =>
  Object.freeze({ ja, en })

export const BULK_INTERSECTION_REPAIR_CONTROL_TEXT =
  Object.freeze({
    repairing: text(
      '一括修復中…',
      'Repairing…',
    ),
    repairAll: text(
      '交差を一括修復（{count}件）',
      'Repair all intersections ({count})',
    ),
    confirmation: text(
      '{count}件の未分割交差を一括修復しますか？',
      'Repair {count} unsplit intersections as one undoable edit?',
    ),
  }) satisfies BulkIntersectionRepairControlText
