import { useState } from 'react'
import { createRoot } from 'react-dom/client'
import { FoldPreview } from '../src/components/FoldPreview.tsx'
import { FoldPreviewCollisionBadge } from '../src/components/FoldPreviewCollisionBadge.tsx'
import {
  MAX_FOLD_PREVIEW_WORLD_SIZE,
  type SingleFoldPreviewModel,
} from '../src/lib/foldPreviewModel.ts'
import { localeStore } from '../src/lib/i18n.ts'
import '../src/App.css'

localeStore.initialize(); localeStore.setLocale('en')
const PROJECT = '018f47a2-4b7a-7cc1-8abc-000000000001'
const LEFT_FACE = '018f47a2-4b7a-7cc1-8abc-000000000002'
const RIGHT_FACE = '018f47a2-4b7a-7cc1-8abc-000000000003'
const HINGE = '018f47a2-4b7a-7cc1-8abc-000000000004'
const HALF_WORLD_SIZE = MAX_FOLD_PREVIEW_WORLD_SIZE / 2
const hinge = {
  edgeId: HINGE,
  leftFaceId: LEFT_FACE,
  rightFaceId: RIGHT_FACE,
  start: { vertexId: '018f47a2-4b7a-7cc1-8abc-000000000005', x: 0, z: -HALF_WORLD_SIZE },
  end: { vertexId: '018f47a2-4b7a-7cc1-8abc-000000000006', x: 0, z: HALF_WORLD_SIZE },
  axis: { x: 0, z: 1 },
  assignment: 'mountain' as const,
  rotationSign: 1 as const,
}
const left = {
  id: LEFT_FACE,
  polygon: [
    { vertexId: '018f47a2-4b7a-7cc1-8abc-000000000007', x: -HALF_WORLD_SIZE, z: -HALF_WORLD_SIZE },
    hinge.start,
    hinge.end,
    { vertexId: '018f47a2-4b7a-7cc1-8abc-000000000008', x: -HALF_WORLD_SIZE, z: HALF_WORLD_SIZE },
  ],
}
const right = {
  id: RIGHT_FACE,
  polygon: [
    hinge.start,
    { vertexId: '018f47a2-4b7a-7cc1-8abc-000000000009', x: HALF_WORLD_SIZE, z: -HALF_WORLD_SIZE },
    { vertexId: '018f47a2-4b7a-7cc1-8abc-000000000010', x: HALF_WORLD_SIZE, z: HALF_WORLD_SIZE },
    hinge.end,
  ],
}
const model = {
  kind: 'single_fold',
  projectId: PROJECT,
  revision: 1,
  worldUnitsPerMillimetre: MAX_FOLD_PREVIEW_WORLD_SIZE / 80,
  paperCenter: { x: 0, y: 0 },
  worldBounds: {
    minX: -HALF_WORLD_SIZE,
    minZ: -HALF_WORLD_SIZE,
    maxX: HALF_WORLD_SIZE,
    maxZ: HALF_WORLD_SIZE,
  },
  faces: [left, right] as const,
  fixedFace: left,
  movingFace: right,
  hinge,
} satisfies SingleFoldPreviewModel
const evidence = { expectedHingeId: HINGE, hingeSelections: [] as (string | null)[], angleRequests: [] as number[] }
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
