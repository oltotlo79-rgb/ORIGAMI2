import type { LocalizedText } from './i18n.ts'

function localized(ja: string, en: string): LocalizedText {
  return Object.freeze({ ja, en })
}

export const INSTRUCTION_AUTO_RECORD_TEXT = Object.freeze({
  stepTitle: localized(
    '自動記録 手順 {step}',
    'Auto-recorded step {step}',
  ),
})
