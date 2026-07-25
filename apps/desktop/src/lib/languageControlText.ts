import type { LocalizedText } from './i18n.ts'

export const LANGUAGE_CONTROL_TEXT: Readonly<Record<'label' | 'japanese' | 'english', LocalizedText>> =
  Object.freeze({
    label: Object.freeze({ ja: '表示言語', en: 'Display language' }),
    japanese: Object.freeze({ ja: '日本語', en: '日本語' }),
    english: Object.freeze({ ja: 'English', en: 'English' }),
  })
