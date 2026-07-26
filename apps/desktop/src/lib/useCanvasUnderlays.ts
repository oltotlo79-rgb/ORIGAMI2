import { useEffect, useMemo, useState } from 'react'

import type { CreaseCanvasUnderlay } from '../components/CreaseCanvas.tsx'
import {
  readUnderlayAssetDataUrl,
  type ProjectSnapshot,
} from './coreClient.ts'

export type CanvasUnderlayImageLoader = (
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  asset: string,
) => Promise<HTMLImageElement>

type LoadedUnderlayImages = Readonly<{
  snapshot: ProjectSnapshot
  imageLoader: CanvasUnderlayImageLoader
  images: ReadonlyMap<string, HTMLImageElement>
}>

async function loadUnderlayImage(
  expectedProjectId: string,
  expectedRevision: number,
  expectedProjectInstanceId: string,
  asset: string,
): Promise<HTMLImageElement> {
  const url = await readUnderlayAssetDataUrl(
    expectedProjectId,
    expectedRevision,
    expectedProjectInstanceId,
    asset,
  )
  const image = new Image()
  await new Promise<void>((resolve, reject) => {
    image.onload = () => resolve()
    image.onerror = () => reject(new Error('underlay image unavailable'))
    image.src = url
  })
  return image
}

export function useCanvasUnderlays(
  snapshot: ProjectSnapshot | null,
  imageLoader: CanvasUnderlayImageLoader = loadUnderlayImage,
): readonly CreaseCanvasUnderlay[] {
  const [loaded, setLoaded] = useState<LoadedUnderlayImages | null>(null)

  useEffect(() => {
    if (!snapshot?.underlays?.underlays.length) {
      setLoaded(null)
      return
    }

    let canceled = false
    const {
      project_id: projectId,
      project_instance_id: projectInstanceId,
      revision,
    } = snapshot
    void Promise.all(snapshot.underlays.underlays.map(async ({ asset }) => [
      asset,
      await imageLoader(projectId, revision, projectInstanceId, asset),
    ] as const)).then((entries) => {
      if (!canceled) {
        setLoaded({
          snapshot,
          imageLoader,
          images: new Map(entries),
        })
      }
    }).catch(() => {
      if (!canceled) setLoaded(null)
    })

    return () => {
      canceled = true
    }
  }, [imageLoader, snapshot])

  return useMemo(() => {
    if (
      !snapshot?.underlays
      || loaded?.snapshot !== snapshot
      || loaded.imageLoader !== imageLoader
    ) return []
    const layers = new Map(
      snapshot.project_layers.layers.map((layer) => [layer.id, layer]),
    )
    return snapshot.underlays.underlays.flatMap((record) => {
      const layer = layers.get(record.layer)
      const image = loaded.images.get(record.asset)
      if (
        !image
        || !layer
        || layer.content_kind !== 'underlay'
        || !layer.visible
      ) return []
      return [{
        id: record.id,
        image,
        x: record.transform.position.x,
        y: record.transform.position.y,
        scaleX: record.transform.scale_x,
        scaleY: record.transform.scale_y,
        rotationDegrees: record.transform.rotation_degrees,
        opacity: record.opacity * layer.opacity,
      }]
    })
  }, [imageLoader, loaded, snapshot])
}
