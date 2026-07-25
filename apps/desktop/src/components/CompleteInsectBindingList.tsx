import type { BeginnerGenerationConstraintsV1 } from '../lib/coreClient'
import {
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
  type LocalizedText,
} from '../lib/i18n.ts'
import {
  COMPLETE_INSECT_BINDING_LIST_TEXT as TEXT,
} from '../lib/completeInsectBindingListText.ts'

type Protrusion = NonNullable<BeginnerGenerationConstraintsV1['protrusions']>[number]

export function CompleteInsectBindingList({ locale, protrusions }: {
  locale: Locale
  protrusions: readonly Protrusion[]
}) {
  const valid = protrusions.length === 5
    && new Set(protrusions.map((target) => target.id)).size === 5
    && protrusions.every((target, index) => target.id === index + 1
      && target.count === 2 && target.symmetry === 'bilateral')
    && protrusions[0]?.direction_milli[0] !== 0
    && protrusions[0]?.direction_milli[1] === 0
    && protrusions[1]?.direction_milli[0] === 0
    && protrusions[1]?.direction_milli[1] !== 0
    && protrusions.slice(2).every((target, index, legs) => index === 0
      || legs[index - 1]!.position_tenths_mm[1] < target.position_tenths_mm[1])
  if (!valid) return null

  const labels: readonly LocalizedText[] = [
    TEXT.wingPair,
    TEXT.antennaPair,
    TEXT.legPair1,
    TEXT.legPair2,
    TEXT.legPair3,
  ]
  return <ol aria-label={selectLocalizedText(locale, TEXT.listAriaLabel)}>
    {protrusions.map((target, index) => <li key={target.id}>
      {formatLocalizedText(locale, TEXT.bindingRow, {
        label: selectLocalizedText(locale, labels[index]!),
        bindingId: target.id,
        length: target.length_tenths_mm,
        thickness: target.thickness_tenths_mm,
      })}
    </li>)}
  </ol>
}
