/** Back-to-front editor layers. Auxiliary marks stay below structural creases. */
export const CANVAS_LINE_KINDS = [
  'auxiliary',
  'mountain',
  'valley',
  'boundary',
  'cut',
] as const

export type CanvasLineKind = (typeof CANVAS_LINE_KINDS)[number]

export type CanvasLineDrawBatch<Line> = Readonly<{
  kind: CanvasLineKind
  selected: boolean
  opacity: number
  layerOrder: number
  lines: readonly Line[]
}>

/**
 * Groups paths into canonical crease-kind layers, keeping source order inside
 * each layer. Valid intersections are covered by their vertex marker; the
 * stable layer order lets large documents use bounded stroke calls. Selected
 * paths are emitted last so their highlight cannot disappear below an ordinary
 * stroke.
 */
export function createCanvasLineDrawBatches<Line extends {
  id: string
  kind: CanvasLineKind
  opacity?: number
  layerOrder?: number
}>(
  lines: readonly Line[],
  selectedLineId: string | null,
): CanvasLineDrawBatch<Line>[] {
  const ordinary = new Map<string, StyleGroup<Line>>()
  const selected = new Map<string, StyleGroup<Line>>()

  for (const line of lines) {
    const groups = line.id === selectedLineId ? selected : ordinary
    const layerOrder = normalizeLayerOrder(line.layerOrder)
    const opacity = normalizeOpacity(line.opacity)
    const key = `${layerOrder}:${opacity}`
    let group = groups.get(key)
    if (!group) {
      group = {
        layerOrder,
        opacity,
        sequence: groups.size,
        buckets: createEmptyBuckets<Line>(),
      }
      groups.set(key, group)
    }
    group.buckets.get(line.kind)?.push(line)
  }

  const batches: CanvasLineDrawBatch<Line>[] = []
  appendStyleGroups(batches, ordinary, false)
  appendStyleGroups(batches, selected, true)
  return batches
}

type StyleGroup<Line> = {
  layerOrder: number
  opacity: number
  sequence: number
  buckets: Map<CanvasLineKind, Line[]>
}

function createEmptyBuckets<Line>() {
  return new Map<CanvasLineKind, Line[]>(
    CANVAS_LINE_KINDS.map((kind) => [kind, []]),
  )
}

function appendStyleGroups<Line>(
  target: CanvasLineDrawBatch<Line>[],
  groups: ReadonlyMap<string, StyleGroup<Line>>,
  selected: boolean,
) {
  const orderedGroups = [...groups.values()].sort((first, second) =>
    first.layerOrder - second.layerOrder || first.sequence - second.sequence)
  for (const group of orderedGroups) {
    for (const kind of CANVAS_LINE_KINDS) {
      const lines = group.buckets.get(kind)
      if (lines?.length) {
        target.push({
          kind,
          selected,
          opacity: group.opacity,
          layerOrder: group.layerOrder,
          lines,
        })
      }
    }
  }
}

function normalizeLayerOrder(value: number | undefined) {
  return Number.isSafeInteger(value) && (value ?? -1) >= 0
    ? value as number
    : 0
}

function normalizeOpacity(value: number | undefined) {
  return typeof value === 'number' && Number.isFinite(value)
    ? Math.min(1, Math.max(0, Object.is(value, -0) ? 0 : value))
    : 1
}
