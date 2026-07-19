import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const appSource = readSource('../src/App.tsx')
const clientSource = readSource('../src/lib/coreClient.ts')
const dialogSource = readSource('../src/components/StaticMeshExportDialog.tsx')
const nativeSource = readSource('../src-tauri/src/lib.rs')
const nativeExportSource = readSource('../src-tauri/src/mesh_export.rs')

test('toolbar exposes a completed-pose-only 3D export dialog', () => {
  assert.match(appSource, /ref=\{meshExportButtonRef\}/u)
  assert.match(appSource, /appliedFoldPose\.state === 'running'/u)
  assert.match(appSource, /\|\| meshExportOpen/u)
  assert.match(appSource, /\{meshExportOpen && \(\s*<StaticMeshExportDialog/u)
  assert.match(appSource, /ja: '3D書出し', en: 'Export 3D'/u)
})

test('native IPC stages bytes privately and exposes no path or geometry arrays', () => {
  for (const command of [
    'preview_static_mesh_export',
    'save_static_mesh_export',
    'cancel_static_mesh_export',
  ]) {
    assert.match(clientSource, new RegExp(command, 'u'))
    assert.match(nativeSource, new RegExp(command, 'u'))
  }
  assert.match(nativeSource, /\.manage\(StaticMeshExportState::default\(\)\)/u)
  const response = sliceBetween(
    nativeExportSource,
    'struct StaticMeshExportPreviewSnapshot',
    'pub(super) struct StaticMeshExportSaveResponse',
  )
  assert.doesNotMatch(response, /\bbytes\b|\bpath\b|positions|normals|triangles\s*:/u)
  assert.match(nativeExportSource, /bytes: Arc<\[u8\]>/u)
  assert.match(nativeExportSource, /persist_export_bytes_to_destination/u)
})

test('save request binds project instance, revision, fingerprint, and pose generation', () => {
  assert.match(
    clientSource,
    /expectedProjectInstanceId: preview\.projectInstanceId[\s\S]*expectedProjectId: preview\.projectId[\s\S]*expectedRevision: preview\.revision[\s\S]*expectedSourceFingerprint: preview\.sourceFingerprint[\s\S]*expectedPoseGeneration: preview\.poseGeneration/u,
  )
  assert.match(nativeExportSource, /pending_is_current\(project, pending\)/u)
  assert.match(
    nativeExportSource,
    /revalidate_current_applied_pose_capability\(project, &pending\.pose_capability\)/u,
  )
  assert.match(nativeExportSource, /view\.generation\(\) == pending\.pose_generation/u)
})

test('UI explicitly discloses mid-surface-only and all STL limitations', () => {
  assert.match(dialogSource, /紙の「中央面」だけ/u)
  assert.match(dialogSource, /閉じた多様体/u)
  assert.match(dialogSource, /guaranteed printable model/u)
  assert.match(dialogSource, /staticMeshExportWarningMessage/u)
  assert.match(nativeExportSource, /StlTriangleSoupFacetNormals/u)
  assert.match(
    readSource('../src/lib/staticMeshExport.ts'),
    /頂点indexと頂点法線を保持しません[\s\S]*triangle soup[\s\S]*facet normal/u,
  )
  assert.match(dialogSource, /warningsAcknowledged/u)
  assert.match(dialogSource, /aria-modal="true"/u)
})

function readSource(relativePath: string) {
  return readFileSync(new URL(relativePath, import.meta.url), 'utf8')
}

function sliceBetween(source: string, start: string, end: string) {
  const startIndex = source.indexOf(start)
  const endIndex = source.indexOf(end, startIndex)
  assert.notEqual(startIndex, -1)
  assert.notEqual(endIndex, -1)
  return source.slice(startIndex, endIndex)
}
