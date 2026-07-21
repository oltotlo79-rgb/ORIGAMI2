import type { BeginnerGenerationConstraintsV1 } from '../lib/coreClient'

type Protrusion = NonNullable<BeginnerGenerationConstraintsV1['protrusions']>[number]

export function CompleteInsectBindingList({ locale, protrusions }: {
  locale: 'ja' | 'en'
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

  const labels = locale === 'ja'
    ? ['翼の組', '触角の組', '脚の組1', '脚の組2', '脚の組3']
    : ['Wing pair', 'Antenna pair', 'Leg pair 1', 'Leg pair 2', 'Leg pair 3']
  return <ol aria-label={locale === 'ja'
    ? '完全昆虫の五組binding寸法'
    : 'Five complete-insect binding dimensions'}>
    {protrusions.map((target, index) => <li key={target.id}>
      {locale === 'ja'
        ? `${labels[index]}・binding ${target.id}・長さ ${target.length_tenths_mm}・厚さ ${target.thickness_tenths_mm}`
        : `${labels[index]} · binding ${target.id} · length ${target.length_tenths_mm} · thickness ${target.thickness_tenths_mm}`}
    </li>)}
  </ol>
}
