import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const app = source('../src/App.tsx')
const client = source('../src/lib/coreClient.ts')
const panel = source('../src/components/ProjectLayerPanel.tsx')

const layerMutations = [
  ['createProjectLayer', 'create_project_layer'],
  ['renameProjectLayer', 'rename_project_layer'],
  ['moveProjectLayer', 'move_project_layer'],
  ['deleteProjectLayer', 'delete_project_layer'],
  ['assignEdgeToProjectLayer', 'assign_edge_to_project_layer'],
] as const

test('App admits layer documents before rendering a fail-closed panel', () => {
  assert.match(
    app,
    /normalizeProjectLayerDocument\(\s*snapshot\.project_layers,\s*snapshot\.crease_pattern\.edges,\s*\)/u,
  )
  assert.match(
    app,
    /project_layers:\s*projectLayers\s*\?\?\s*DEFAULT_PROJECT_LAYER_DOCUMENT_V1/u,
  )
  assert.match(
    app,
    /setProjectLayerDocumentInvalid\(layerDocumentInvalid\)/u,
  )
  assert.match(
    app,
    /<ProjectLayerPanel[\s\S]*?document=\{nativeSnapshot\.project_layers\}[\s\S]*?documentInvalid=\{projectLayerDocumentInvalid\}[\s\S]*?onCreate=\{createLayerFromPanel\}[\s\S]*?onRename=\{renameLayerFromPanel\}[\s\S]*?onMove=\{moveLayerFromPanel\}[\s\S]*?onDelete=\{deleteLayerFromPanel\}[\s\S]*?onAssignSelectedEdge=\{assignSelectedEdgeToLayer\}/u,
  )
})

test('App supplies the exact admitted base snapshot to each layer mutation', () => {
  assert.match(
    app,
    /const runProjectLayerEdit = useCallback\([\s\S]*?baseSnapshot:\s*ProjectSnapshot[\s\S]*?baseSnapshot\.project_instance_id !== projectInstanceId[\s\S]*?baseSnapshot\.project_id !== projectId[\s\S]*?baseSnapshot\.revision !== revision[\s\S]*?return action\(\s*projectId,\s*revision,\s*projectInstanceId,\s*baseSnapshot,\s*\)/u,
  )
  for (const [clientFunction] of layerMutations) {
    const call = app.indexOf(`${clientFunction}(`)
    assert.ok(call >= 0, `${clientFunction} App call`)
    assert.match(
      app.slice(call, call + 320),
      /projectId,\s*revision,\s*projectInstanceId,\s*baseSnapshot,/u,
    )
  }
})

test('strict clients admit only the layer delta into the current snapshot', () => {
  for (const [clientFunction, nativeCommand] of layerMutations) {
    const section = typescriptFunctionSection(client, clientFunction)
    assert.match(section, /baseSnapshot:\s*ProjectSnapshot/u)
    assert.match(
      section,
      new RegExp(
        String.raw`invoke<unknown>\('${nativeCommand}'[\s\S]*?admitProjectLayerMutationSnapshot\(\s*value,\s*baseSnapshot,`,
        'u',
      ),
    )
  }
  assert.match(
    client,
    /paper:\s*base\.paper,\s*crease_pattern:\s*base\.crease_pattern,\s*instruction_timeline:\s*base\.instruction_timeline,\s*numeric_expressions:\s*base\.numeric_expressions,\s*geometric_constraints:\s*base\.geometric_constraints,\s*project_layers:\s*projectLayers,/u,
  )
})

test('the panel describes drawing order without claiming physical layer order', () => {
  assert.match(panel, /描画順/u)
  assert.match(panel, /drawing order/u)
  assert.doesNotMatch(panel, /積層順|stacking order/iu)
})

function typescriptFunctionSection(text: string, functionName: string) {
  const start = text.indexOf(`export function ${functionName}(`)
  assert.ok(start >= 0, `${functionName} function`)
  const nextExport = text.indexOf('\nexport function ', start + 1)
  return text.slice(start, nextExport < 0 ? text.length : nextExport)
}

function source(relativePath: string) {
  return readFileSync(new URL(relativePath, import.meta.url), 'utf8')
}
