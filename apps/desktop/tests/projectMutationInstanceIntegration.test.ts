import assert from 'node:assert/strict'
import { readFileSync, readdirSync } from 'node:fs'
import test from 'node:test'

const app = [
  source('../src/App.tsx'),
  source('../src/lib/appText.ts'),
].join('\n')
const instructionPanel = source('../src/components/InstructionTimelinePanel.tsx')
const client = source('../src/lib/coreClient.ts')
const native = source('../src-tauri/src/lib.rs')
const beginnerDesignNative = source('../src-tauri/src/beginner_design_commands.rs')
const geometricConstraintNative = source('../src-tauri/src/geometric_constraint_commands.rs')
const patternEditNative = source('../src-tauri/src/pattern_edit_commands.rs')
const projectLifecycleNative = source('../src-tauri/src/project_lifecycle_commands.rs')
const nativeMutationSources = [
  native,
  beginnerDesignNative,
  geometricConstraintNative,
  patternEditNative,
  projectLifecycleNative,
] as const
const formats = source('../../../crates/ori-formats/src/lib.rs')
const nativeUnitTestsPath = new URL('../src-tauri/src/tests.rs', import.meta.url).pathname
const nativeRustSources = rustSources(new URL('../src-tauri/src/', import.meta.url))
const nativeHandler = rustInvokeHandlerSection(native)

const mutationContracts = [
  ['addInstructionStep', 'add_instruction_step'],
  ['updateInstructionStepMetadata', 'update_instruction_step_metadata'],
  ['replaceInstructionStepPose', 'replace_instruction_step_pose'],
  ['removeInstructionStep', 'remove_instruction_step'],
  ['moveInstructionStep', 'move_instruction_step'],
  ['duplicateInstructionStep', 'duplicate_instruction_step'],
  ['splitInstructionStep', 'split_instruction_step'],
  ['mergeAdjacentInstructionSteps', 'merge_adjacent_instruction_steps'],
  [
    'appendNamedTechniqueInstructionSteps',
    'append_named_technique_instruction_steps',
  ],
  ['appendGenericTreeInstructionProposal', 'append_generic_tree_instruction_proposal'],
  ['addVertex', 'add_vertex'],
  ['addConnectedVertex', 'add_connected_vertex'],
  ['addEdge', 'add_edge'],
  ['addRayToFirstTarget', 'add_ray_to_first_target'],
  ['moveVertex', 'move_vertex'],
  ['moveEdge', 'move_edge'],
  ['mirrorEdgeLeftRight', 'mirror_edge_left_right'],
  ['rotateEdgeAboutPoint', 'rotate_edge_about_point'],
  ['moveVertices', 'move_vertices'],
  ['applyGeometricConstraintSolve', 'apply_geometric_constraint_solve'],
  ['applyMirrorSelection', 'apply_mirror_selection'],
  ['confirmLinearArray', 'confirm_linear_array'],
  ['confirmRadialArray', 'confirm_radial_array'],
  ['removeVertex', 'remove_vertex'],
  ['removeBoundaryVertex', 'remove_boundary_vertex'],
  ['removeEdge', 'remove_edge'],
  ['createProjectLayer', 'create_project_layer'],
  ['renameProjectLayer', 'rename_project_layer'],
  ['updateProjectLayerPresentation', 'update_project_layer_presentation'],
  ['moveProjectLayer', 'move_project_layer'],
  ['deleteProjectLayer', 'delete_project_layer'],
  ['assignEdgeToProjectLayer', 'assign_edge_to_project_layer'],
  ['addEdgeOrientationConstraint', 'add_edge_orientation_constraint'],
  ['addGeometricConstraint', 'add_geometric_constraint'],
  ['removeGeometricConstraint', 'remove_geometric_constraint'],
  ['addAnnotation', 'add_annotation'],
  ['updateAnnotation', 'update_annotation'],
  ['removeAnnotation', 'remove_annotation'],
  ['addUnderlay', 'add_underlay'],
  ['updateUnderlay', 'update_underlay'],
  ['removeUnderlay', 'remove_underlay'],
  ['importUnderlayImage', 'import_underlay_image'],
  ['undo', 'undo'],
  ['redo', 'redo'],
  ['setCuttingAllowed', 'set_cutting_allowed'],
  ['setElementMetadata', 'set_element_metadata'],
  ['updateProjectMemo', 'update_project_memo'],
  ['updateBeginnerDesignProfile', 'update_beginner_design_profile'],
  ['updateBeginnerReferenceConsensus', 'update_beginner_reference_consensus'],
  ['importBeginnerReferenceModel', 'import_beginner_reference_model'],
  ['activateBeginnerReferenceModelAsset', 'activate_beginner_reference_model_asset'],
  ['archiveBeginnerReferenceModelAsset', 'archive_beginner_reference_model_asset'],
  ['applyBeginnerReferenceModelFeatures', 'apply_beginner_reference_model_features'],
  ['applyBeginnerGeneratedPlan', 'apply_beginner_generated_plan'],
  ['applyBeginnerParameterGridCandidate', 'apply_beginner_parameter_grid_candidate'],
  ['applyBeginnerSymmetricParameters', 'apply_beginner_symmetric_parameters'],
  ['updatePaperProperties', 'update_paper_properties'],
  ['importFrontPaperTexture', 'import_front_paper_texture'],
  ['importBackPaperTexture', 'import_back_paper_texture'],
  ['setLengthDisplayUnit', 'set_length_display_unit'],
  ['resizeRectangularPaper', 'resize_rectangular_paper'],
  ['splitBoundaryEdge', 'split_boundary_edge'],
  ['splitEdge', 'split_edge'],
  ['connectEdgeIntersection', 'connect_edge_intersection'],
  ['connectIntersectionCluster', 'connect_intersection_cluster'],
  ['connectTJunction', 'connect_t_junction'],
] as const

test('the revision-changing mutation contract matrix remains complete', () => {
  assert.equal(mutationContracts.length, 66)
  assert.equal(new Set(mutationContracts.map(([name]) => name)).size, 66)
  assert.equal(new Set(mutationContracts.map(([, command]) => command)).size, 66)
  assert.deepEqual(
    productionRevisionChangingCommands(nativeMutationSources),
    mutationContracts.map(([, command]) => command).toSorted(),
  )
})

for (const [clientFunction, nativeCommand] of mutationContracts) {
  test(`${clientFunction} carries the open-instance binding through its native payload`, () => {
    const clientFunctionSource = typescriptFunctionSection(client, clientFunction)
    if (clientFunction === 'appendNamedTechniqueInstructionSteps') {
      assert.match(
        clientFunctionSource,
        /export function appendNamedTechniqueInstructionSteps\(\s*guard:\s*ProjectOccGuard,/u,
      )
      assert.match(
        clientFunctionSource,
        /projectOccGuardField\(\s*guard,\s*'expectedProjectInstanceId',\s*\)[\s\S]*projectOccGuardField\(guard, 'expectedProjectId'\)[\s\S]*projectOccGuardField\(guard, 'expectedRevision'\)/u,
      )
    } else {
      assert.match(
        clientFunctionSource,
        new RegExp(
          String.raw`export function ${clientFunction}\(\s*expectedProjectId:\s*string,\s*expectedRevision:\s*number,\s*expectedProjectInstanceId:\s*string,?`,
          'u',
        ),
      )
    }
    assert.match(
      clientFunctionSource,
      new RegExp(String.raw`invoke(?:<[^>]+>)?\('${nativeCommand}'`, 'u'),
    )
    assert.match(
      clientFunctionSource,
      /\{\s*expectedProjectInstanceId,\s*expectedProjectId,\s*expectedRevision,/u,
    )

    const nativeFunctionSource = rustFunctionSectionFromSources(
      nativeMutationSources,
      nativeCommand,
    )
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

test('ProjectExpectation is the only production command and project-expectation funnel', () => {
  assert.match(
    native,
    /struct ProjectExpectation\s*\{\s*instance_id:\s*ProjectId,\s*project_id:\s*ProjectId,\s*revision:\s*u64,\s*\}/u,
  )

  const commandFunnel = rustFunctionSection(native, 'execute_expected_command')
  assert.match(
    commandFunnel,
    /execute_command\(\s*project,\s*expectation\.instance_id,\s*expectation\.project_id,\s*expectation\.revision,\s*command,\s*\)/u,
  )
  const expectationFunnel = rustFunctionSection(native, 'ensure_project_expectation')
  assert.match(
    expectationFunnel,
    /ensure_expected_project\(\s*project,\s*expectation\.instance_id,\s*expectation\.project_id,\s*expectation\.revision,\s*\)/u,
  )
  const lockFunnel = rustFunctionSection(native, 'lock_and_expect')
  assert.match(
    lockFunnel,
    /let project = lock_project\(state\)\?;[\s\S]*ensure_project_expectation\(&project, expectation\)\?;[\s\S]*Ok\(project\)/u,
  )

  for (const [path, text] of nativeRustSources) {
    const production = rustCodeWithoutLineComments(rustProductionSection(text))
    const isRoot = path.endsWith('/src-tauri/src/lib.rs')
    assert.equal(
      [...production.matchAll(/\bexecute_command\s*\(/gu)].length,
      isRoot ? 2 : 0,
      `${path} must use execute_expected_command outside the root definition and its delegation`,
    )
    assert.equal(
      [...production.matchAll(/\bensure_expected_project\s*\(/gu)].length,
      isRoot ? 2 : 0,
      `${path} must use ensure_project_expectation outside the root definition and its delegation`,
    )
  }
})

test('project JSON read and write share one ordered document validator', () => {
  const writer = rustFunctionSection(formats, 'write_project_json_with_size_limit')
  const reader = rustFunctionSection(formats, 'read_project_json_with_limits')
  const validator = rustFunctionSection(formats, 'validate_project_document')
  assert.match(
    writer,
    /validate_project_document\(document\)\?;[\s\S]*serde_json::to_vec_pretty\(document\)\?/u,
  )
  assert.match(
    reader,
    /ensure_project_json_size\(bytes\.len\(\), limits\.max_input_size\)\?;[\s\S]*serde_json::from_slice\(bytes\)\?;[\s\S]*validate_project_document\(&document\)\?;/u,
  )
  assert.match(
    validator,
    /validate_project_envelope\(document\)\?;\s*validate_project_geometry_finiteness\(document\)\?;\s*validate_instruction_timeline\(&document\.instruction_timeline\)\?;\s*validate_numeric_expressions\(&document\.numeric_expressions\)\?;\s*validate_current_vertex_expression_bindings\(document\)\?;\s*validate_project_geometric_constraints\(document\)\?;\s*validate_project_layer_document_against_pattern_v1\(&document\.layers, &document\.crease_pattern\)\?;\s*validate_project_annotations\(document\)\?;\s*validate_project_underlays\(document\)\?;/u,
  )
  assert.equal(
    occurrences(rustProductionSection(formats), 'validate_project_document('),
    3,
  )
})

test('instruction pose analysis binds both capture and commit to the open instance', () => {
  const analysis = rustFunctionSection(native, 'analyze_instruction_pose')
  assert.match(
    analysis,
    /lock_and_expect\(\s*state,\s*ProjectExpectation::new\(\s*expected_project_instance_id,\s*expected_project_id,\s*expected_revision,\s*\),\s*\)\?/u,
  )
  const finish = rustFunctionSection(native, 'finish_instruction_pose')
  assert.match(
    finish,
    /ensure_project_expectation\(\s*project,\s*ProjectExpectation::new\(\s*expected_project_instance_id,\s*expected_project_id,\s*expected_revision,\s*\),\s*\)\?/u,
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
  assert.equal(boundCallbacks.length, 8)
  for (const [clientFunction] of mutationContracts.slice(0, 8)) {
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

function productionRevisionChangingCommands(texts: readonly string[]) {
  const productions = texts.map(rustProductionSection)
  const productionFunctions = [
    ...new Set(productions.flatMap((production) => [
      ...production.matchAll(
        /\n(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn ([a-z][a-z0-9_]*)\(/gu,
      ),
    ].map((match) => match[1]!))),
  ]
  const revisionChanging = new Map<string, boolean>()
  const reachesRevisionChange = (name: string, active: Set<string>): boolean => {
    const cached = revisionChanging.get(name)
    if (cached !== undefined) return cached
    if (active.has(name)) return false
    const section = rustFunctionSectionFromSources(productions, name)
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
  const commands = productions
    .flatMap((production) => [
      ...production.matchAll(
        /\n#\[tauri::command\]\n(?:#\[[^\n]+\]\n)*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn ([a-z][a-z0-9_]*)\(/gu,
      ),
    ].map((match) => match[1]!))
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
  const match = new RegExp(
    String.raw`\n(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn ${name}\(`,
    'u',
  ).exec(text)
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

function rustFunctionSectionFromSources(texts: readonly string[], name: string) {
  const declaration = new RegExp(
    String.raw`\n(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn ${name}\(`,
    'u',
  )
  const matches = texts.filter((text) => declaration.test(text))
  assert.equal(matches.length, 1, `${name} must have one native definition`)
  return rustFunctionSection(matches[0]!, name)
}

function rustProductionSection(text: string) {
  const testModule = /\n#\[cfg\(test\)\]\n(?:pub(?:\([^)]*\))?\s+)?mod tests(?:\s*\{|;)/u.exec(text)
  return testModule ? text.slice(0, testModule.index) : text
}

function rustCodeWithoutLineComments(text: string) {
  return text.replace(/\/\/[^\r\n]*/gu, '')
}

function rustSources(directory: URL): Array<readonly [string, string]> {
  const result: Array<readonly [string, string]> = []
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const child = new URL(`${entry.name}${entry.isDirectory() ? '/' : ''}`, directory)
    if (entry.isDirectory()) {
      result.push(...rustSources(child))
    } else if (
      entry.isFile()
      && entry.name.endsWith('.rs')
      && child.pathname !== nativeUnitTestsPath
    ) {
      result.push([child.pathname, readFileSync(child, 'utf8')])
    }
  }
  return result
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
