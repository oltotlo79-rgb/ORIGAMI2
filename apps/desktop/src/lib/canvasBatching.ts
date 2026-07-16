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
}>(
  lines: readonly Line[],
  selectedLineId: string | null,
): CanvasLineDrawBatch<Line>[] {
  const ordinary = createEmptyBuckets<Line>()
  const selected = createEmptyBuckets<Line>()

  for (const line of lines) {
    const buckets = line.id === selectedLineId ? selected : ordinary
    buckets.get(line.kind)?.push(line)
  }

  const batches: CanvasLineDrawBatch<Line>[] = []
  appendNonEmptyBatches(batches, ordinary, false)
  appendNonEmptyBatches(batches, selected, true)
  return batches
}

function createEmptyBuckets<Line>() {
  return new Map<CanvasLineKind, Line[]>(
    CANVAS_LINE_KINDS.map((kind) => [kind, []]),
  )
}

function appendNonEmptyBatches<Line>(
  target: CanvasLineDrawBatch<Line>[],
  buckets: ReadonlyMap<CanvasLineKind, Line[]>,
  selected: boolean,
) {
  for (const kind of CANVAS_LINE_KINDS) {
    const lines = buckets.get(kind)
    if (lines?.length) target.push({ kind, selected, lines })
  }
}
