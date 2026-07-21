import type { BeginnerGenerationConstraintsV1 } from '../lib/coreClient'

type Protrusion = NonNullable<BeginnerGenerationConstraintsV1['protrusions']>[number]

export function CompleteAnimalBindingList({ locale, protrusions }: {
  locale: 'ja' | 'en'
  protrusions: readonly Protrusion[]
}) {
  const valid = protrusions.length === 4
    && new Set(protrusions.map((target) => target.id)).size === 4
    && protrusions[0]?.count === 1 && protrusions[0].symmetry === 'none'
    && protrusions[0].direction_milli[0] === 0 && protrusions[0].direction_milli[1] !== 0
    && protrusions[1]?.count === 1 && protrusions[1].symmetry === 'none'
    && protrusions[1].direction_milli[0] !== 0 && protrusions[1].direction_milli[1] === 0
    && protrusions[2]?.count === 2 && protrusions[2].symmetry === 'bilateral'
    && protrusions[3]?.count === 4 && protrusions[3].symmetry === 'bilateral'
  if (!valid) return null

  return (
    <ol aria-label={locale === 'ja' ? '完全動物の四部位binding寸法' : 'Four complete-animal binding dimensions'}>
      {protrusions.map((target) => (
        <li key={target.id}>
          {locale === 'ja'
            ? `binding ${target.id}・数 ${target.count}・長さ ${target.length_tenths_mm}・厚さ ${target.thickness_tenths_mm}`
            : `Binding ${target.id} · count ${target.count} · length ${target.length_tenths_mm} · thickness ${target.thickness_tenths_mm}`}
        </li>
      ))}
    </ol>
  )
}
