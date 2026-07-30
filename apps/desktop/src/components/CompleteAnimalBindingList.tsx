import type { BeginnerGenerationConstraintsV1 } from '../lib/coreClient'
import {
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
} from '../lib/i18n.ts'
import {
  COMPLETE_ANIMAL_BINDING_LIST_TEXT as TEXT,
} from '../lib/completeAnimalBindingListText.ts'
import { resolveCompleteAnimalBindings } from '../lib/completeAnimalBindings.ts'

type Protrusion = NonNullable<BeginnerGenerationConstraintsV1['protrusions']>[number]

export function CompleteAnimalBindingList({ locale, protrusions }: {
  locale: Locale
  protrusions: readonly Protrusion[]
}) {
  const bindings = resolveCompleteAnimalBindings(
    protrusions,
    protrusions.length === 5,
  )
  if (!bindings) return null
  const partCount = selectLocalizedText(
    locale,
    bindings.wing ? TEXT.fivePartCount : TEXT.fourPartCount,
  )

  return (
    <ol aria-label={formatLocalizedText(locale, TEXT.ariaLabel, {
      partCount,
    })}>
      {bindings.ordered.map((target) => (
        <li key={target.id}>
          {formatLocalizedText(locale, TEXT.bindingRow, {
            id: target.id,
            count: target.count,
            length: target.length_tenths_mm,
            thickness: target.thickness_tenths_mm,
          })}
        </li>
      ))}
    </ol>
  )
}
