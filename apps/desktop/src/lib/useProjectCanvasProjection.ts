import { useMemo } from 'react'

import type {
  ProjectSnapshot,
  ProjectTopologyResponse,
} from './coreClient.ts'
import { buildFoldPreviewModel } from './foldPreviewModel.ts'
import {
  collectBoundaryLengthReferences,
  resolveLengthDisplayUnit,
} from './lengthUnit.ts'
import {
  resolvePaperBounds,
  resolvePaperPolygon,
  resolveRectangularPaperSize,
} from './appGeometry.ts'
import {
  createCanvasAnnotations,
  createCanvasFaces,
} from './projectCanvasProjection.ts'

export function useProjectCanvasProjection(
  snapshot: ProjectSnapshot | null,
  topologyResponse: ProjectTopologyResponse | null,
) {
  const boundaryVertexIds = useMemo(
    () => new Set(snapshot?.paper.boundary_vertices ?? []),
    [snapshot],
  )
  const paperBounds = useMemo(
    () => resolvePaperBounds(snapshot),
    [snapshot],
  )
  const paperPolygon = useMemo(
    () => resolvePaperPolygon(snapshot),
    [snapshot],
  )
  const boundaryLengthReferences = useMemo(
    () => collectBoundaryLengthReferences(snapshot),
    [snapshot],
  )
  const lengthDisplayUnit = useMemo(
    () => resolveLengthDisplayUnit(snapshot),
    [snapshot],
  )
  const rectangularPaperSize = useMemo(
    () => resolveRectangularPaperSize(snapshot),
    [snapshot],
  )
  const foldPreviewModel = useMemo(
    () => buildFoldPreviewModel(snapshot, topologyResponse),
    [snapshot, topologyResponse],
  )
  const canvasFaces = useMemo(
    () => createCanvasFaces(snapshot, topologyResponse),
    [snapshot, topologyResponse],
  )
  const canvasAnnotations = useMemo(
    () => createCanvasAnnotations(snapshot),
    [snapshot],
  )

  return {
    boundaryVertexIds,
    paperBounds,
    paperPolygon,
    boundaryLengthReferences,
    lengthDisplayUnit,
    rectangularPaperSize,
    foldPreviewModel,
    canvasFaces,
    canvasAnnotations,
  } as const
}
