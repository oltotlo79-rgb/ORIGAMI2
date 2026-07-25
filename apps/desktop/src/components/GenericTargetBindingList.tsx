import type { BeginnerGenerationConstraintsV1 } from '../lib/coreClient'
import {
  GENERIC_TARGET_BINDING_LIST_TEXT as TEXT,
} from '../lib/genericTargetBindingListText.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
} from '../lib/i18n.ts'

type Protrusion = NonNullable<BeginnerGenerationConstraintsV1['protrusions']>[number]

export function GenericTargetBindingList({ locale, protrusions }: {
  locale: 'ja' | 'en'
  protrusions: readonly Protrusion[]
}) {
  const valid = protrusions.length >= 2 && protrusions.length <= 8
    && protrusions.every((target, index) => target.id === index + 1
      && (target.count === 1 && target.symmetry === 'none'
        || (target.count === 2 || target.count === 4) && target.symmetry === 'bilateral'))
  if (!valid) return null
  return <ol aria-label={selectLocalizedText(locale, TEXT.ariaLabel)}>
    {protrusions.map((target) => <li key={target.id}>
      {formatLocalizedText(locale, TEXT.bindingRow, {
        id: target.id,
        symmetry: selectLocalizedText(locale, target.symmetry === 'none'
          ? TEXT.symmetryAsymmetric
          : TEXT.symmetryBilateral),
        count: target.count,
        length: target.length_tenths_mm,
        thickness: target.thickness_tenths_mm,
      })}
    </li>)}
  </ol>
}
