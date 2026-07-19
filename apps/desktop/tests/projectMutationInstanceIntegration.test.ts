import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const app = source('../src/App.tsx')
const instructionPanel = source('../src/components/InstructionTimelinePanel.tsx')
const client = source('../src/lib/coreClient.ts')
const native = source('../src-tauri/src/lib.rs')
const nativeHandler = rustInvokeHandlerSection(native)

const mutationContracts = [
  ['addInstructionStep', 'add_instruction_step'],
  ['updateInstructionStepMetadata', 'update_instruction_step_metadata'],
  ['replaceInstructionStepPose', 'replace_instruction_step_pose'],
  ['removeInstructionStep', 'remove_instruction_step'],
  ['moveInstructionStep', 'move_instruction_step'],
  ['addVertex', 'add_vertex'],
  ['addEdge', 'add_edge'],
  ['moveVertex', 'move_vertex'],
  ['removeVertex', 'remove_vertex'],
  ['removeBoundaryVertex', 'remove_boundary_vertex'],
  ['removeEdge', 'remove_edge'],
  ['addEdgeOrientationConstraint', 'add_edge_orientation_constraint'],
  ['removeGeometricConstraint', 'remove_geometric_constraint'],
  ['undo', 'undo'],
  ['redo', 'redo'],
  ['setCuttingAllowed', 'set_cutting_allowed'],
  ['updatePaperProperties', 'update_paper_properties'],
  ['setLengthDisplayUnit', 'set_length_display_unit'],
  ['resizeRectangularPaper', 'resize_rectangular_paper'],
  ['splitBoundaryEdge', 'split_boundary_edge'],
  ['splitEdge', 'split_edge'],
  ['connectEdgeIntersection', 'connect_edge_intersection'],
  ['connectIntersectionCluster', 'connect_intersection_cluster'],
  ['connectTJunction', 'connect_t_junction'],
] as const

test('the revision-changing mutation contract matrix remains complete', () => {
  assert.equal(mutationContracts.length, 24)
  assert.equal(new Set(mutationContracts.map(([name]) => name)).size, 24)
  assert.equal(new Set(mutationContracts.map(([, command]) => command)).size, 24)
  assert.deepEqual(
    productionRevisionChangingCommands(native),
    mutationContracts.map(([, command]) => command).toSorted(),
  )
})

for (const [clientFunction, nativeCommand] of mutationContracts) {
  test(`${clientFunction} carries the open-instance binding through its native payload`, () => {
    const clientFunctionSource = typescriptFunctionSection(client, clientFunction)
    assert.match(
      clientFunctionSource,
      new RegExp(
        String.raw`export function ${clientFunction}\(\s*expectedProjectId:\s*string,\s*expectedRevision:\s*number,\s*expectedProjectInstanceId:\s*string,?`,
        'u',
      ),
    )
    assert.match(
      clientFunctionSource,
      new RegExp(String.raw`invoke(?:<[^>]+>)?\('${nativeCommand}'`, 'u'),
    )
    assert.match(
      clientFunctionSource,
      /\{\s*expectedProjectInstanceId,\s*expectedProjectId,\s*expectedRevision,/u,
    )

    const nativeFunctionSource = rustFunctionSection(native, nativeCommand)
    assert.match(
      nativeFunctionSource,
      /expected_project_instance_id:\s*ProjectId,\s*expected_project_id:\s*ProjectId,\s*expected_revision:\s*u64,/u,
    )
    assert.ok(
      occurrences(nativeFunctionSource, 'expected_project_instance_id') >= 2,
      `${nativeCommand} must forward, not merely declare, the instance binding`,
    )
    assert.equal(
      (
        nativeHandler.match(
          new RegExp(String.raw`^\s*${nativeCommand},\s*$`, 'gmu'),
        ) ?? []
      ).length,
      1,
      `${nativeCommand} must be registered exactly once in the invoke handler`,
    )
  })
}

test('all central edit history paths reject a foreign instance before state access', () => {
  for (const functionName of ['execute_command', 'execute_undo', 'execute_redo']) {
    const section = rustFunctionSection(native, functionName)
    const identityCheck = section.indexOf('ensure_project_instance_identity(')
    const editorAccess = section.indexOf('project.editor.')
    assert.ok(identityCheck >= 0, `${functionName} must check the open instance`)
    assert.ok(
      editorAccess > identityCheck,
      `${functionName} must check the open instance before editor state`,
    )
  }

  const identityGuard = rustFunctionSection(native, 'ensure_project_instance_identity')
  assert.match(
    identityGuard,
    /project\.instance_id != expected_instance_id[\s\S]*?return Err\("the open project instance changed while the file dialog was open"\.to_owned\(\)\)/u,
  )
  assert.doesNotMatch(identityGuard, /format!\s*\(/u)
})

test('instruction pose analysis binds both capture and commit to the open instance', () => {
  const analysis = rustFunctionSection(native, 'analyze_instruction_pose')
  assert.match(
    analysis,
    /ensure_expected_project\(\s*&project,\s*expected_project_instance_id,\s*expected_project_id,\s*expected_revision,\s*\)\?/u,
  )
  const finish = rustFunctionSection(native, 'finish_instruction_pose')
  assert.match(
    finish,
    /ensure_expected_project\(\s*project,\s*expected_project_instance_id,\s*expected_project_id,\s*expected_revision,\s*\)\?/u,
  )
  assert.match(
    finish,
    /project\.instance_id != analyzed\.project_instance_id/u,
  )

  for (const command of ['add_instruction_step', 'replace_instruction_step_pose']) {
    const section = rustFunctionSection(native, command)
    for (const stage of ['analyze_instruction_pose', 'finish_instruction_pose']) {
      assert.match(
        section,
        new RegExp(
          String.raw`${stage}\([\s\S]*?expected_project_instance_id,[\s\S]*?expected_project_id,[\s\S]*?expected_revision,`,
          'u',
        ),
      )
    }
  }
})

test('App binds every edit callback and verifies the returned instance snapshot', () => {
  assert.match(
    app,
    /action:\s*\(\s*projectId:\s*string,\s*revision:\s*number,\s*projectInstanceId:\s*string,\s*\)\s*=>\s*Promise<ProjectSnapshot>/u,
  )
  assert.match(
    app,
    /await action\(\s*current\.project_id,\s*current\.revision,\s*current\.project_instance_id,\s*\)/u,
  )
  assert.match(
    app,
    /isExpectedNativeEditSnapshot\(\s*snapshot,\s*current\.project_instance_id,\s*current\.project_id,\s*current\.revision,\s*\)/u,
  )
  assert.doesNotMatch(
    app,
    /runNativeEdit\(\s*(?:async\s*)?\(\s*projectId\s*,\s*revision\s*\)\s*=>/u,
  )
  assert.match(app, /<InstructionTimelinePanel[\s\S]*?runNativeEdit=\{runNativeEdit\}/u)
})

test('InstructionTimelinePanel requires and forwards the instance binding', () => {
  assert.match(
    instructionPanel,
    /runNativeEdit\(\s*action:\s*\(\s*projectId:\s*string,\s*revision:\s*number,\s*projectInstanceId:\s*string,\s*\)\s*=>\s*Promise<ProjectSnapshot>/u,
  )
  assert.doesNotMatch(
    instructionPanel,
    /runNativeEdit\(\s*(?:async\s*)?\(\s*projectId\s*,\s*revision\s*\)\s*=>/u,
  )
  const boundCallbacks = instructionPanel.match(
    /runNativeEdit\(\s*(?:async\s*)?\(\s*projectId\s*,\s*revision\s*,\s*projectInstanceId\s*\)\s*=>/gu,
  ) ?? []
  assert.equal(boundCallbacks.length, 5)
  for (const [clientFunction] of mutationContracts.slice(0, 5)) {
    const callIndex = instructionPanel.indexOf(`${clientFunction}(`)
    assert.ok(callIndex >= 0, `${clientFunction} panel call`)
    assert.match(
      instructionPanel.slice(callIndex, callIndex + 300),
      /projectId,\s*revision,\s*projectInstanceId,/u,
    )
  }
})

function occurrences(text: string, value: string) {
  return text.split(value).length - 1
}

function productionRevisionChangingCommands(text: string) {
  const testModule = text.indexOf('\n#[cfg(test)]\nmod tests')
  assert.ok(testModule >= 0, 'native production/test boundary')
  const production = text.slice(0, testModule)
  const productionFunctions = [
    ...production.matchAll(/\n(?:async\s+)?fn ([a-z][a-z0-9_]*)\(/gu),
  ].map((match) => match[1]!)
  const revisionChanging = new Map<string, boolean>()
  const reachesRevisionChange = (name: string, active: Set<string>): boolean => {
    const cached = revisionChanging.get(name)
    if (cached !== undefined) return cached
    if (active.has(name)) return false
    const section = rustFunctionSection(production, name)
    if (/\bexecute_(?:command|undo|redo)\(/u.test(section)) {
      revisionChanging.set(name, true)
      return true
    }
    const nextActive = new Set(active).add(name)
    const result = productionFunctions.some((candidate) => (
      candidate !== name
      && new RegExp(String.raw`\b${candidate}\s*\(`, 'u').test(section)
      && reachesRevisionChange(candidate, nextActive)
    ))
    revisionChanging.set(name, result)
    return result
  }
  const commands = [
    ...production.matchAll(
      /\n#\[tauri::command\]\n(?:#\[[^\n]+\]\n)*(?:async\s+)?fn ([a-z][a-z0-9_]*)\(/gu,
    ),
  ]
    .map((match) => match[1]!)
    .filter((name) => reachesRevisionChange(name, new Set()))
    .toSorted()
  assert.equal(new Set(commands).size, commands.length)
  return commands
}

function rustInvokeHandlerSection(text: string) {
  const marker = 'tauri::generate_handler!['
  const start = text.indexOf(marker)
  assert.ok(start >= 0, 'native invoke handler')
  const end = text.indexOf('])', start + marker.length)
  assert.ok(end >= 0, 'native invoke handler closing delimiter')
  return text.slice(start, end + 2)
}

function rustFunctionSection(text: string, name: string) {
  const match = new RegExp(String.raw`\n(?:async\s+)?fn ${name}\(`, 'u').exec(text)
  assert.ok(match, `${name} native function`)
  const start = match.index + 1
  const openingBrace = text.indexOf('{', start)
  assert.ok(openingBrace >= 0, `${name} opening brace`)
  let depth = 0
  for (let index = openingBrace; index < text.length; index += 1) {
    if (text[index] === '{') depth += 1
    if (text[index] === '}') {
      depth -= 1
      if (depth === 0) return text.slice(start, index + 1)
    }
  }
  assert.fail(`${name} closing brace`)
}

function typescriptFunctionSection(text: string, name: string) {
  const startMarker = `export function ${name}(`
  const start = text.indexOf(startMarker)
  assert.ok(start >= 0, `${name} client function`)
  const next = text.indexOf('\nexport function ', start + startMarker.length)
  return text.slice(start, next < 0 ? text.length : next)
}

function source(relativePath: string) {
  return readFileSync(new URL(relativePath, import.meta.url), 'utf8')
}
