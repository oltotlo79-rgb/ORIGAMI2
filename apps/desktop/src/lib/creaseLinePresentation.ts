export const CREASE_LINE_KINDS = [
  'boundary',
  'mountain',
  'valley',
  'auxiliary',
  'cut',
] as const

export type CreaseLineKind = (typeof CREASE_LINE_KINDS)[number]

export type CreaseLinePattern =
  | 'solid'
  | 'dash-dot'
  | 'dash'
  | 'dot'
  | 'dash-dot-dot'

export type CreaseLinePresentation = Readonly<{
  color: string
  pattern: CreaseLinePattern
  canvasDash: readonly number[]
  lineCap: 'butt' | 'round'
  lineWidth: number
}>

/**
 * Screen colours are an additional cue. `canvasDash` and `lineCap` are the
 * colour-independent source of truth, so every kind stays distinguishable
 * when a display or printer maps all strokes to black.
 */
export const CREASE_LINE_PRESENTATIONS: Readonly<
  Record<CreaseLineKind, CreaseLinePresentation>
> = {
  boundary: {
    color: '#23303f',
    pattern: 'solid',
    canvasDash: [],
    lineCap: 'butt',
    lineWidth: 2.5,
  },
  mountain: {
    color: '#d95252',
    pattern: 'dash-dot',
    canvasDash: [10, 3, 2, 3],
    lineCap: 'butt',
    lineWidth: 1.8,
  },
  valley: {
    color: '#3678d4',
    pattern: 'dash',
    canvasDash: [5, 3],
    lineCap: 'butt',
    lineWidth: 1.8,
  },
  auxiliary: {
    color: '#7b8794',
    pattern: 'dot',
    canvasDash: [1, 3],
    lineCap: 'round',
    lineWidth: 1.8,
  },
  cut: {
    color: '#a85d00',
    pattern: 'dash-dot-dot',
    canvasDash: [10, 3, 2, 3, 2, 3],
    lineCap: 'butt',
    lineWidth: 2.5,
  },
}
