import type { BeginnerGenerationConstraintsV1 } from '../lib/coreClient'

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
  return <ol aria-label={locale === 'ja'
    ? '上限付き汎用対象binding寸法'
    : 'Bounded generic target binding dimensions'}>
    {protrusions.map((target) => <li key={target.id}>
      {locale === 'ja'
        ? `binding ${target.id}・${target.symmetry === 'none' ? '非対称単独' : '左右対称'}・数 ${target.count}・長さ ${target.length_tenths_mm}・厚さ ${target.thickness_tenths_mm}`
        : `Binding ${target.id} · ${target.symmetry === 'none' ? 'asymmetric single' : 'bilateral'} · count ${target.count} · length ${target.length_tenths_mm} · thickness ${target.thickness_tenths_mm}`}
    </li>)}
  </ol>
}
