import { useState } from 'react'
import { createRoot } from 'react-dom/client'
import { FoldPreview } from '../src/components/FoldPreview.tsx'
import { FoldPreviewCollisionBadge } from '../src/components/FoldPreviewCollisionBadge.tsx'
import { localeStore } from '../src/lib/i18n.ts'
import '../src/App.css'

localeStore.initialize(); localeStore.setLocale('en')
const hinge = { edgeId: 'hinge-main', start: { vertexId: 'b', x: 0, z: -40 }, end: { vertexId: 'c', x: 0, z: 40 }, axis: { x: 0, z: 1 }, assignment: 'mountain' as const, rotationSign: 1 as const }
const left = { id: 'left', polygon: [{ vertexId: 'a', x: -40, z: -40 }, hinge.start, hinge.end, { vertexId: 'd', x: -40, z: 40 }] }
const right = { id: 'right', polygon: [hinge.start, { vertexId: 'e', x: 40, z: -40 }, { vertexId: 'f', x: 40, z: 40 }, hinge.end] }
const model = { kind: 'single_fold' as const, projectId: 'browser-fixture', revision: 1, worldUnitsPerMillimetre: 1, paperCenter: { x: 0, y: 0 }, worldBounds: { minX: -40, minZ: -40, maxX: 40, maxZ: 40 }, faces: [left, right], fixedFace: left, movingFace: right, hinge }
const evidence = { hingeSelections: [] as (string | null)[], angleRequests: [] as number[] }
Object.assign(window, { __ORIGAMI2_FOLD_PREVIEW_EVIDENCE__: evidence })

function Harness() {
  const [selected, setSelected] = useState<string | null>(null)
  return <><FoldPreview angle={0} model={model} selectedHingeId={selected} thicknessMm={0.1}
      localeStore={localeStore}
      onSelectHinge={(id) => { evidence.hingeSelections.push(id); setSelected(id) }}
      onRequestFoldAngle={(angle) => { evidence.angleRequests.push(angle) }} />
    <FoldPreviewCollisionBadge summary={{ kind: 'unavailable', requestKey: 'browser-block' }} description="Browser blocking fixture" localeStore={localeStore} />
  </>
}
createRoot(document.getElementById('root')!).render(<Harness />)
