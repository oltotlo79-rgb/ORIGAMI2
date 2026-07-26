import type { LocalizedText } from './i18n.ts'

type LengthUnitPresentationTextKey =
  | 'numberLocale'
  | 'paperEdgeRatio'
  | 'unavailable'

function localized(ja: string, en: string): LocalizedText {
  return Object.freeze({ ja, en })
}

export const LENGTH_UNIT_PRESENTATION_TEXT: Readonly<
  Record<LengthUnitPresentationTextKey, LocalizedText>
> = Object.freeze({
  numberLocale: localized('ja-JP', 'en-US'),
  paperEdgeRatio: localized('紙辺比', 'paper-edge ratio'),
  unavailable: localized('計測不可', 'Unavailable'),
})
