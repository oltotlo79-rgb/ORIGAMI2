export const BUILTIN_PAPER_PATTERNS = Object.freeze({
  dots: '00000000-0000-0000-0000-00000000a001',
  grid: '00000000-0000-0000-0000-00000000a002',
  stripes: '00000000-0000-0000-0000-00000000a003',
} as const)

export type BuiltinPaperPattern = keyof typeof BUILTIN_PAPER_PATTERNS

export function builtinPaperPatternFromAsset(
  assetId: string | null | undefined,
): BuiltinPaperPattern | null {
  if (!assetId) return null
  const normalized = assetId.toLowerCase()
  for (const [pattern, id] of Object.entries(BUILTIN_PAPER_PATTERNS)) {
    if (normalized === id) return pattern as BuiltinPaperPattern
  }
  return null
}

export function builtinPaperPatternAsset(
  value: FormDataEntryValue | null,
): string | null {
  if (typeof value !== 'string' || value === 'none') return null
  return BUILTIN_PAPER_PATTERNS[value as BuiltinPaperPattern] ?? null
}

export function paperPatternCss(
  assetId: string | null | undefined,
  color: string,
): string | undefined {
  switch (builtinPaperPatternFromAsset(assetId)) {
    case 'dots':
      return `radial-gradient(circle, color-mix(in srgb, ${color} 35%, #333) 1.4px, transparent 1.6px)`
    case 'grid':
      return `linear-gradient(color-mix(in srgb, ${color} 45%, #555) 1px, transparent 1px), linear-gradient(90deg, color-mix(in srgb, ${color} 45%, #555) 1px, transparent 1px)`
    case 'stripes':
      return `repeating-linear-gradient(135deg, transparent 0 7px, color-mix(in srgb, ${color} 55%, #777) 7px 9px)`
    default:
      return undefined
  }
}
