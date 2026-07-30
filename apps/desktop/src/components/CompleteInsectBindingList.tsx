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
import { resolveCompleteInsectBindings } from '../lib/completeInsectBindings.ts'

type Protrusion = NonNullable<BeginnerGenerationConstraintsV1['protrusions']>[number]

export function CompleteInsectBindingList({ locale, protrusions }: {
  locale: Locale
  protrusions: readonly Protrusion[]
}) {
  const bindings = resolveCompleteInsectBindings(protrusions)
  if (!bindings) return null

  const labels: readonly LocalizedText[] = [
    TEXT.wingPair,
    TEXT.antennaPair,
    TEXT.legPair1,
    TEXT.legPair2,
    TEXT.legPair3,
  ]
  return <ol aria-label={selectLocalizedText(locale, TEXT.listAriaLabel)}>
    {bindings.ordered.map((target, index) => <li key={target.id}>
      {formatLocalizedText(locale, TEXT.bindingRow, {
        label: selectLocalizedText(locale, labels[index]!),
        bindingId: target.id,
        length: target.length_tenths_mm,
        thickness: target.thickness_tenths_mm,
      })}
    </li>)}
  </ol>
}
