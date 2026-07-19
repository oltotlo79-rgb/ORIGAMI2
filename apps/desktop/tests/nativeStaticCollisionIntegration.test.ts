import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const app = source('../src/App.tsx')
const preview = source('../src/components/FoldPreview.tsx')
const client = source('../src/lib/nativeStaticCollisionNative.ts')
const coreClient = source('../src/lib/coreClient.ts')
const nativeLib = source('../src-tauri/src/lib.rs')
const nativePose = source('../src-tauri/src/applied_pose.rs')

test('the rendered pose reaches native apply, bound inspection, and the visible badge', () => {
  assert.match(
    app,
    /nativeStaticCollisionCoordinator\s*\.inspectLatest\(current\.request\)/,
  )
  assert.match(app, /createNativeStaticCollisionInspectionCoordinator\(/)
  assert.match(app, /projectInstanceId:\s*project\.project_instance_id/)
  assert.match(app, /completeHingeAngles:\s*pose\.hingeAngles\.map/)
  assert.match(app, /selectBoundNativeStaticCollisionView\(/)
  assert.match(app, /nativeCollisionState=\{/)
  assert.match(app, /nativeCollisionObservedPose=\{appliedFoldPose\}/)
  assert.match(app, /setNativeStaticCollisionRetrySequence/)
  assert.match(
    app,
    /setBoundNativeStaticCollisionView\(\{\s*requestKey: current\.requestKey,\s*view: \{ kind: 'checking' \},\s*\}\)/,
  )
  assert.match(preview, /<PoseBoundNativeStaticCollisionBadge/)
  assert.match(preview, /onRetry=\{onRetryNativeCollision\}/)
  assert.match(preview, /renderedPose=\{renderedAppliedPose\}/)

  const applyIndex = client.indexOf('const appliedBinding = await transport.applyPose(pose)')
  const inspectIndex = client.indexOf('const inspection = await transport.inspect()')
  const bindingIndex = client.indexOf('nativeStaticCollisionBindingsEqual(')
  assert.ok(applyIndex >= 0)
  assert.ok(inspectIndex > applyIndex)
  assert.ok(bindingIndex > inspectIndex)
})

test('frontend and native register the same closed pose and diagnosis contracts', () => {
  assert.match(coreClient, /project_instance_id:\s*string/)
  assert.match(client, /'apply_current_native_pose'/)
  assert.match(client, /'inspect_current_static_collision'/)
  assert.match(nativeLib, /async fn apply_current_native_pose/)
  assert.match(nativeLib, /async fn inspect_current_static_collision/)
  assert.match(nativeLib, /apply_current_native_pose,\s*inspect_current_static_collision,/)
  assert.match(nativePose, /serialize_with = "serialize_u64_decimal"/)
  assert.match(nativePose, /serializer\.collect_str\(value\)/)
})

function source(relativePath: string) {
  return readFileSync(new URL(relativePath, import.meta.url), 'utf8')
}
