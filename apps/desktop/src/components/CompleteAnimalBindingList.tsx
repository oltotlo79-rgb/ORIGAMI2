import type { BeginnerGenerationConstraintsV1 } from '../lib/coreClient'
import {
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
} from '../lib/i18n.ts'
import {
  COMPLETE_ANIMAL_BINDING_LIST_TEXT as TEXT,
} from '../lib/completeAnimalBindingListText.ts'

type Protrusion = NonNullable<BeginnerGenerationConstraintsV1['protrusions']>[number]

export function CompleteAnimalBindingList({ locale, protrusions }: {
  locale: Locale
  protrusions: readonly Protrusion[]
}) {
  const valid = (protrusions.length === 4 || protrusions.length === 5)
    && new Set(protrusions.map((target) => target.id)).size === protrusions.length
    && protrusions[0]?.count === 1 && protrusions[0].symmetry === 'none'
    && protrusions[0].direction_milli[0] === 0 && protrusions[0].direction_milli[1] !== 0
    && protrusions[1]?.count === 1 && protrusions[1].symmetry === 'none'
    && protrusions[1].direction_milli[0] !== 0 && protrusions[1].direction_milli[1] === 0
    && protrusions[2]?.count === 2 && protrusions[2].symmetry === 'bilateral'
    && protrusions[3]?.count === 4 && protrusions[3].symmetry === 'bilateral'
    && (protrusions.length === 4
      || (protrusions[4]?.count === 2 && protrusions[4].symmetry === 'bilateral'))
  if (!valid) return null
  const partCount = selectLocalizedText(
    locale,
    protrusions.length === 5 ? TEXT.fivePartCount : TEXT.fourPartCount,
  )

  return (
    <ol aria-label={formatLocalizedText(locale, TEXT.ariaLabel, {
      partCount,
    })}>
      {protrusions.map((target) => (
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
