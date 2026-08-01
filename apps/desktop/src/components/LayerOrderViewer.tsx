import {
  selectLocalizedText,
  type Locale,
  type LocalizedText,
} from '../lib/i18n.ts'
import type { LayerOrderViewerCell } from '../lib/currentLayerOrderView.ts'
import { STACKED_FOLD_PANEL_TEXT as TEXT } from '../lib/stackedFoldPanelText.ts'

export type LayerOrderViewerScope =
  | 'global-flat-result'
  | 'stacked-fold-proposal'

export type LayerOrderViewerProps = Readonly<{
  locale: Locale
  scope: LayerOrderViewerScope
  cells: readonly LayerOrderViewerCell[]
  selectedCell: string | null
  selectedFace: string | null
  hoveredFace: string | null
  onSelectCell(value: string): void
  onSelectFace(value: string): void
  onHoverFace(value: string | null): void
}>

export function LayerOrderViewer({
  locale,
  scope,
  cells,
  selectedCell,
  selectedFace,
  hoveredFace,
  onSelectCell,
  onSelectFace,
  onHoverFace,
}: LayerOrderViewerProps) {
  const text = (localized: LocalizedText) =>
    selectLocalizedText(locale, localized)
  const active = cells.find(
    (cell) => cell.cellKeySha256 === selectedCell,
  ) ?? cells[0]
  if (!active) return null
  const polygon = projectBoundaryToXzSchematic(active.boundaryWorld)
  const heading = scope === 'stacked-fold-proposal'
    ? TEXT.flatEndpointProposalLayerSchematic
    : TEXT.flatResultLayerSchematic
  const Heading = scope === 'stacked-fold-proposal' ? 'h3' : 'h4'
  const detail = scope === 'stacked-fold-proposal'
    ? TEXT.flatEndpointProposalLayerSchematicDetail
    : TEXT.flatResultLayerSchematicDetail
  const layerOffsetStep = active.bottomToTopFaces.length > 1
    ? Math.min(9, 24 / (active.bottomToTopFaces.length - 1))
    : 0

  return (
    <section
      className="layer-stack-schematic"
      data-layer-schematic-scope={scope}
      aria-label={text(heading)}
    >
      <Heading>{text(heading)}</Heading>
      <p className="muted">
        {text(detail)}
      </p>
      <ol className="layer-stack-schematic-cells">
        {cells.map((cell, index) => (
          <li key={cell.cellKeySha256}>
            <button
              type="button"
              aria-label={`${text(TEXT.cell)} ${index + 1}`}
              aria-pressed={cell.cellKeySha256 === active.cellKeySha256}
              onClick={() => onSelectCell(cell.cellKeySha256)}
            >
              {text(TEXT.cell)} {index + 1}
            </button>
          </li>
        ))}
      </ol>
      <svg
        viewBox="0 0 240 180"
        aria-hidden="true"
        focusable="false"
      >
        {active.bottomToTopFaces.map((face, index) => {
          const offset =
            (active.bottomToTopFaces.length - 1 - index) * layerOffsetStep
          const highlighted = face === selectedFace || face === hoveredFace
          const position = active.bottomToTopFaces.length === 1
            ? text(TEXT.onlyLayer)
            : index === 0
              ? text(TEXT.backBottom)
              : index === active.bottomToTopFaces.length - 1
                ? text(TEXT.frontTop)
                : text(TEXT.middle)
          return (
            <polygon
              key={`${face}:${index}`}
              points={polygon}
              transform={`translate(${offset} ${-offset})`}
              fill={highlighted
                ? '#f6b73c'
                : `hsl(${205 + index * 22} 55% 62%)`}
              fillOpacity="0.72"
              stroke={highlighted ? '#6b3e00' : '#29465b'}
            >
              <title>
                {position} · {text(TEXT.layer)} {index + 1}
              </title>
            </polygon>
          )
        })}
      </svg>
      <ol className="layer-stack-schematic-layers">
        {active.bottomToTopFaces.map((face, index) => (
          <li key={`${face}:${index}`}>
            <button
              type="button"
              aria-pressed={face === selectedFace}
              onMouseEnter={() => onHoverFace(face)}
              onMouseLeave={() => onHoverFace(null)}
              onFocus={() => onHoverFace(face)}
              onBlur={() => onHoverFace(null)}
              onClick={() => onSelectFace(face)}
            >
              {active.bottomToTopFaces.length === 1
                ? text(TEXT.onlyLayer)
                : index === 0
                  ? text(TEXT.backBottom)
                  : index === active.bottomToTopFaces.length - 1
                    ? text(TEXT.frontTop)
                    : text(TEXT.middle)} · {text(TEXT.layer)} {index + 1}
            </button>
          </li>
        ))}
      </ol>
    </section>
  )
}

function projectBoundaryToXzSchematic(
  boundary: readonly (readonly [number, number, number])[],
): string {
  let maximumMagnitude = 0
  for (const point of boundary) {
    maximumMagnitude = Math.max(
      maximumMagnitude,
      Math.abs(point[0]),
      Math.abs(point[2]),
    )
  }
  const divisor = maximumMagnitude === 0 ? 1 : maximumMagnitude
  let minX = Number.POSITIVE_INFINITY
  let maxX = Number.NEGATIVE_INFINITY
  let minZ = Number.POSITIVE_INFINITY
  let maxZ = Number.NEGATIVE_INFINITY
  for (const point of boundary) {
    const x = point[0] / divisor
    const z = point[2] / divisor
    minX = Math.min(minX, x)
    maxX = Math.max(maxX, x)
    minZ = Math.min(minZ, z)
    maxZ = Math.max(maxZ, z)
  }
  const spanX = maxX - minX
  const spanZ = maxZ - minZ
  const scaleX = spanX === 0 ? Number.POSITIVE_INFINITY : 160 / spanX
  const scaleZ = spanZ === 0 ? Number.POSITIVE_INFINITY : 90 / spanZ
  const finiteScale = Math.min(scaleX, scaleZ)
  const scale = Number.isFinite(finiteScale) ? finiteScale : 1
  const width = spanX * scale
  const height = spanZ * scale
  const originX = 28 + (160 - width) / 2
  const originY = 42 + (90 - height) / 2
  return boundary.map((point) => {
    const x = point[0] / divisor
    const z = point[2] / divisor
    return `${originX + (x - minX) * scale},${originY + (z - minZ) * scale}`
  }).join(' ')
}
