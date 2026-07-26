import type { CreaseLine } from '../components/CreaseCanvas'
import type { FoldTechniqueFileDocumentV1 } from './foldTechniqueEditor'
import type { Locale } from './i18n'

export function isEditingText(target: EventTarget | null) {
  if (!(target instanceof HTMLElement)) return false
  if (target.matches('input, textarea')) return true
  return target.isContentEditable || Boolean(target.closest('[contenteditable="true"]'))
}

export function nextFoldTechniqueRequestId(reference: { current: number }): number {
  const next = reference.current >= 0xffff_ffff
    ? 1
    : reference.current + 1
  reference.current = next
  return next
}

export function selectedNamedBookFold(
  document: FoldTechniqueFileDocumentV1 | null,
  techniqueIndex: number,
  locale: Locale,
  selectedCreaseKind?: CreaseLine['kind'],
) {
  const technique = document?.techniques[techniqueIndex]
  if (!technique) return null
  const physical = technique.operations.filter(
    (operation) => [
      'straight_line_stacked_fold', 'inside_reverse_fold', 'outside_reverse_fold',
      'sink_fold',
      'layer_selective_manipulation',
    ].includes(operation.action.kind),
  )
  const isReverse = physical[0]?.action.kind === 'inside_reverse_fold'
    || physical[0]?.action.kind === 'outside_reverse_fold'
  const isAccordion = physical.length >= 3
    && physical.every((operation) => operation.action.kind === 'straight_line_stacked_fold')
  const isSink = physical[0]?.action.kind === 'sink_fold'
  const isLayer = physical[0]?.action.kind === 'layer_selective_manipulation'
  const isMountain = selectedCreaseKind === 'mountain'
  const isValley = selectedCreaseKind === 'valley'
  const isCrimp = physical.length === 2
    && physical.every((operation) => operation.action.kind === 'straight_line_stacked_fold')
  if ((!isAccordion && !isCrimp && physical.length !== 1) || (!isReverse && !isAccordion && !isCrimp && !isSink && !isLayer && technique.operations.some(
      (operation) => operation.execution_support.status
        === 'unsupported_physical_operation',
    ))) return null
  return Object.freeze({
    document,
    techniqueId: technique.id,
    name: technique.names.find((entry) => entry.locale === locale)?.text
      ?? technique.names.find((entry) => entry.locale === 'ja')?.text
      ?? technique.names[0]?.text
      ?? technique.id,
    kind: isAccordion ? 'accordion' as const
      : physical[0]?.action.kind === 'inside_reverse_fold' ? 'inside_reverse' as const
      : physical[0]?.action.kind === 'outside_reverse_fold' ? 'outside_reverse' as const
      : isCrimp ? 'crimp' as const
      : isSink ? 'sink' as const : isLayer ? 'layer_selective' as const
      : isMountain ? 'mountain' as const : isValley ? 'valley' as const
      : 'book' as const,
  })
}

export function namedBookFoldPalette(
  document: FoldTechniqueFileDocumentV1 | null,
  locale: Locale,
  selectedCreaseKind?: CreaseLine['kind'],
) {
  if (!document) return []
  return document.techniques.map((technique, index) => {
    const admitted = selectedNamedBookFold(document, index, locale, selectedCreaseKind)
    return Object.freeze({
      techniqueId: technique.id,
      name: technique.names.find((entry) => entry.locale === locale)?.text
        ?? technique.names.find((entry) => entry.locale === 'ja')?.text
        ?? technique.names[0]?.text
        ?? technique.id,
      supported: admitted !== null && admitted.techniqueId === technique.id,
    })
  })
}
