import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const creaseNative = source('../src-tauri/src/crease_export.rs')
const creaseDialog = source('../src/components/CreaseExportDialog.tsx')
const instructionCore = source('../../../crates/ori-formats/src/instruction_export.rs')
const instructionNative = source('../src-tauri/src/instruction_export.rs')
const instructionDialog = source('../src/components/InstructionExportDialog.tsx')
const meshNative = source('../src-tauri/src/mesh_export.rs')
const meshDialog = source('../src/components/StaticMeshExportDialog.tsx')

test('every current export family discloses its losses before saving', () => {
  for (const format of ['Fold', 'Svg', 'Pdf', 'Dxf']) {
    assert.match(creaseNative, new RegExp(`CreaseExportFormatRequest::${format}`, 'u'))
  }
  assert.match(creaseNative, /fn export_warnings\(/u)
  assert.match(creaseDialog, /preview\.warnings\.map/u)
  assert.match(creaseDialog, /warningsConfirmed/u)

  for (const format of ['Pdf17', 'SvgPageZip']) {
    assert.match(instructionCore, new RegExp(`Self::${format}`, 'u'))
  }
  assert.match(instructionCore, /INSTRUCTION_EXPORT_WARNINGS[^=]*=\s*\[/u)
  assert.match(instructionDialog, /preview\.warnings\.map/u)
  assert.match(instructionDialog, /warningsConfirmed/u)

  for (const format of ['Obj', 'Stl', 'Glb']) {
    assert.match(meshNative, new RegExp(`Self::${format}`, 'u'))
  }
  assert.match(meshNative, /fn export_warnings\(/u)
  assert.match(meshDialog, /preview\.warnings\.map/u)
  assert.match(meshDialog, /warningsAcknowledged/u)
})

test('all native export commits enforce acknowledgement independently of the UI', () => {
  for (const nativeSource of [creaseNative, instructionNative, meshNative]) {
    assert.match(
      nativeSource,
      /require_warning_acknowledgement\([^)]*warnings_acknowledged/u,
    )
    assert.match(
      nativeSource,
      /if !pending\.warnings\.is_empty\(\) && !warnings_acknowledged/u,
    )
  }
})

function source(relativePath: string) {
  return readFileSync(new URL(relativePath, import.meta.url), 'utf8')
}
