import type { LocalizedText } from './i18n.ts'

export const FOLD_PREVIEW_COLLISION_BADGE_TEXT: Readonly<Record<
  'warningAriaLabel' | 'informationAriaLabel' | 'visible',
  LocalizedText
>> = Object.freeze({
  warningAriaLabel: Object.freeze({
    ja: '安全上の警告。表示姿勢。{text}',
    en: 'Safety warning. Current pose. {text}',
  }),
  informationAriaLabel: Object.freeze({
    ja: '衝突情報。表示姿勢。{text}',
    en: 'Collision information. Current pose. {text}',
  }),
  visible: Object.freeze({
    ja: '表示姿勢｜{text}',
    en: 'Current pose | {text}',
  }),
})
