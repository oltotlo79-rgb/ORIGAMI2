import assert from 'node:assert/strict'
import { readFileSync, readdirSync } from 'node:fs'
import test from 'node:test'

const rustStructuralCodeCache = new Map<string, string>()

const app = [
  source('../src/App.tsx'),
  source('../src/lib/appText.ts'),
].join('\n')
const instructionPanel = source('../src/components/InstructionTimelinePanel.tsx')
const fold3dFramesLauncher = source('../src/components/Fold3dFramesLauncher.tsx')
const client = [
  source('../src/lib/coreClient.ts'),
  source('../src/lib/fold3dFrames.ts'),
].join('\n')
const native = source('../src-tauri/src/lib.rs')
const beginnerDesignNative = source('../src-tauri/src/beginner_design_commands.rs')
const beginnerRecognitionNative = source('../src-tauri/src/beginner_recognition.rs')
const fold3dFramesNative = source('../src-tauri/src/fold_3d_frames_import.rs')
const geometricConstraintNative = source('../src-tauri/src/geometric_constraint_commands.rs')
const patternEditNative = source('../src-tauri/src/pattern_edit_commands.rs')
const projectLifecycleNative = source('../src-tauri/src/project_lifecycle_commands.rs')
const nativeMutationSources = [
  native,
  beginnerDesignNative,
  beginnerRecognitionNative,
  fold3dFramesNative,
  geometricConstraintNative,
  patternEditNative,
  projectLifecycleNative,
] as const
const formats = source('../../../crates/ori-formats/src/lib.rs')
const nativeUnitTestsPath = new URL('../src-tauri/src/tests.rs', import.meta.url).pathname
const nativeUnitTestsDirectoryPath = new URL(
  '../src-tauri/src/tests/',
  import.meta.url,
).pathname
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
  ['applyFold3dInstructionTimeline', 'apply_fold_3d_instruction_timeline'],
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
  ['applyBeginnerOutlineCandidate', 'apply_beginner_outline_candidate'],
  ['applyBeginnerPartAssignments', 'apply_beginner_part_assignments'],
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
  assert.equal(mutationContracts.length, 69)
  assert.equal(new Set(mutationContracts.map(([name]) => name)).size, 69)
  assert.equal(new Set(mutationContracts.map(([, command]) => command)).size, 69)
  assert.deepEqual(
    productionRevisionChangingCommands(nativeMutationSources),
    mutationContracts.map(([, command]) => command).toSorted(),
  )
})

test('the production Rust scan excludes only modules reached through cfg(test)', () => {
  assert.equal(
    nativeRustSources.some(([path]) => (
      path === nativeUnitTestsPath
      || path.startsWith(nativeUnitTestsDirectoryPath)
      || path.endsWith('/src-tauri/src/geometry_reference_compat_tests.rs')
      || path.endsWith('/src-tauri/src/stacked_fold_dyadic_scope_tests.rs')
    )),
    false,
  )
  assert.ok(
    nativeRustSources.some(([path]) => (
      path.endsWith('/src-tauri/src/beginner_design_commands.rs')
    )),
  )
  const syntheticSources = [
    [
      '/workspace/src/lib.rs',
      `
#[cfg(test)]
#[path = "nested/tests.rs"]
mod declared_tests;

mod production_tests;

mod shared;

#[cfg(test)]
#[path = "shared.rs"]
mod shared_test;

#[doc = "#[cfg(test)]"]
mod doc_cfg_production;

#[doc = r##"#[path = "doc_path_target.rs"]"##]
mod doc_path_production;

#[cfg(test)]
mod inline_tests {
    include!("inline_tests.rs");
}

#[cfg(test)]
mod nested_inline_tests {
    mod helper;
    #[path = "helper.rs"]
    mod explicit_helper;
}

const MODULE_NAME_IN_A_STRING: &str = r#"
#[cfg(test)]
mod string_named_tests;
"#;

/*
#[cfg(test)]
mod commented_tests;
*/
`,
    ],
    [
      '/workspace/src/nested/tests.rs',
      `
mod helper;
`,
    ],
    ['/workspace/src/nested/tests/helper.rs', 'fn test_helper() {}'],
    [
      '/workspace/src/production_tests.rs',
      'fn production_function_despite_its_name() {}',
    ],
    [
      '/workspace/src/orphan_tests.rs',
      'fn production_function_until_declared_test_only() {}',
    ],
    ['/workspace/src/inline_tests.rs', 'fn included_test_helper() {}'],
    ['/workspace/src/string_named_tests.rs', 'fn production_string_fixture() {}'],
    ['/workspace/src/commented_tests.rs', 'fn production_comment_fixture() {}'],
    ['/workspace/src/shared.rs', 'fn shared_production_helper() {}'],
    ['/workspace/src/doc_cfg_production.rs', 'fn documented_production() {}'],
    ['/workspace/src/doc_path_production.rs', 'fn path_documented_production() {}'],
    ['/workspace/src/doc_path_target.rs', 'fn unrelated_production() {}'],
    ['/workspace/src/helper.rs', 'fn same_named_production_helper() {}'],
    ['/workspace/src/nested_inline_tests/helper.rs', 'fn conservatively_scanned_test() {}'],
  ] as const
  assert.deepEqual(
    productionRustSources(syntheticSources).map(([path]) => path),
    [
      '/workspace/src/lib.rs',
      '/workspace/src/production_tests.rs',
      '/workspace/src/orphan_tests.rs',
      '/workspace/src/string_named_tests.rs',
      '/workspace/src/commented_tests.rs',
      '/workspace/src/shared.rs',
      '/workspace/src/doc_cfg_production.rs',
      '/workspace/src/doc_path_production.rs',
      '/workspace/src/doc_path_target.rs',
      '/workspace/src/helper.rs',
      '/workspace/src/nested_inline_tests/helper.rs',
    ],
  )
  const syntheticProductionModule = `
fn invalid_production_mutation_path() {
    execute_command(project, instance_id, project_id, revision, command);
}
`
  assert.equal(
    [
      ...rustCodeWithoutLineComments(
        rustProductionSection(syntheticProductionModule),
      ).matchAll(/\bexecute_command\s*\(/gu),
    ].length,
    1,
  )
})

test('the production Rust scan fails closed on conditional path attributes', () => {
  assert.throws(
    () => productionRustSources([
      [
        '/workspace/src/lib.rs',
        `
#[cfg(test)]
#[cfg_attr(test, path = "only_tests.rs")]
mod shared;
`,
      ],
      ['/workspace/src/shared.rs', 'fn production_shared() {}'],
      ['/workspace/src/only_tests.rs', 'fn conditional_test_only() {}'],
    ]),
    /conditional Rust path attributes are unsupported/u,
  )
})

test('the mutation scan follows free Rust calls without treating undo and redo methods as commands', () => {
  const syntheticMethodOnlyCommand = `
fn undo() {
    execute_undo(project, instance_id, project_id, revision);
}

fn redo() {
    execute_redo(project, instance_id, project_id, revision);
}

#[tauri::command]
fn open_project() {
    project.editor.undo();
    project.editor . undo();
    project.editor./* structural spacing */redo();
}
`
  assert.deepEqual(
    productionRevisionChangingCommands([syntheticMethodOnlyCommand]),
    [],
  )

  const syntheticFreeCallCommand = `
fn undo() {
    execute_undo(project, instance_id, project_id, revision);
}

#[tauri::command]
fn explicit_undo() {
    undo();
}
`
  assert.deepEqual(
    productionRevisionChangingCommands([syntheticFreeCallCommand]),
    ['explicit_undo'],
  )
})

test('the mutation scan reaches a revision change through a cyclic call graph', () => {
  const syntheticCyclicCommands = `
fn cycle_a() {
    cycle_b();
    mutate();
}

fn cycle_b() {
    cycle_a();
}

fn mutate() {
    execute_command(project, instance_id, project_id, revision, command);
}

#[tauri::command]
fn command_through_a() {
    cycle_a();
}

#[tauri::command]
fn command_through_b() {
    cycle_b();
}
`
  assert.deepEqual(
    productionRevisionChangingCommands([syntheticCyclicCommands]),
    ['command_through_a', 'command_through_b'],
  )
})

test('the mutation scan uses Rust structure across generics and test modules', () => {
  const syntheticStructuredCommand = `
const COMMENT_LIKE_TEXT: &str = r#"
#[cfg(test)]
mod tests {
    fn fake_mutation() {}
}
"#;

/*
#[cfg(test)]
mod tests {
    fn another_fake_mutation() {}
}
*/

fn generic_mutation<T>() {
    let misleading_closing_brace = "}";
    execute_command(project, instance_id, project_id, revision, command);
}

#[cfg(test)]
mod tests {
    fn test_only_mutation() {
        execute_command(project, instance_id, project_id, revision, command);
    }
}

#[tauri::command]
fn production_command_after_tests() {
    generic_mutation::<()>();
}
`
  assert.deepEqual(
    productionRevisionChangingCommands([syntheticStructuredCommand]),
    ['production_command_after_tests'],
  )
})

const proposalBoundClientContracts = new Map<string, {
  signature: RegExp
  payload: RegExp
}>([
  [
    'applyBeginnerOutlineCandidate',
    {
      signature: /export function applyBeginnerOutlineCandidate\(\s*proposal:\s*BeginnerOutlineCandidatesResponse,/u,
      payload: /\{\s*expectedProjectInstanceId:\s*proposal\.project_instance_id,\s*expectedProjectId:\s*proposal\.project_id,\s*expectedRevision:\s*proposal\.revision,/u,
    },
  ],
  [
    'applyBeginnerPartAssignments',
    {
      signature: /export function applyBeginnerPartAssignments\(\s*outline:\s*BeginnerOutlineCandidatesResponse,/u,
      payload: /\{\s*expectedProjectInstanceId:\s*outline\.project_instance_id,\s*expectedProjectId:\s*outline\.project_id,\s*expectedRevision:\s*outline\.revision,/u,
    },
  ],
])

const nativeRequestStructs = new Map<string, string>([
  ['apply_beginner_outline_candidate', 'ApplyBeginnerOutlineCandidateRequest'],
  ['apply_beginner_part_assignments', 'ApplyBeginnerPartAssignmentsRequest'],
  ['apply_fold_3d_instruction_timeline', 'Fold3dTimelineRequest'],
])

for (const [clientFunction, nativeCommand] of mutationContracts) {
  test(`${clientFunction} carries the open-instance binding through its native payload`, () => {
    const clientFunctionSource = typescriptFunctionSection(client, clientFunction)
    const proposalBinding = proposalBoundClientContracts.get(clientFunction)
    if (clientFunction === 'appendNamedTechniqueInstructionSteps') {
      assert.match(
        clientFunctionSource,
        /export function appendNamedTechniqueInstructionSteps\(\s*guard:\s*ProjectOccGuard,/u,
      )
      assert.match(
        clientFunctionSource,
        /projectOccGuardField\(\s*guard,\s*'expectedProjectInstanceId',\s*\)[\s\S]*projectOccGuardField\(guard, 'expectedProjectId'\)[\s\S]*projectOccGuardField\(guard, 'expectedRevision'\)/u,
      )
    } else if (proposalBinding) {
      assert.match(clientFunctionSource, proposalBinding.signature)
      assert.match(clientFunctionSource, proposalBinding.payload)
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
    if (!proposalBinding) {
      assert.match(
        clientFunctionSource,
        /\{\s*expectedProjectInstanceId,\s*expectedProjectId,\s*expectedRevision,/u,
      )
    }

    const nativeFunctionSource = rustFunctionSectionFromSources(
      nativeMutationSources,
      nativeCommand,
    )
    const nativeRequestStruct = nativeRequestStructs.get(nativeCommand)
    const bindingDeclarationSource = nativeRequestStruct
      ? rustStructSectionFromSources(nativeMutationSources, nativeRequestStruct)
      : nativeFunctionSource
    assert.match(
      bindingDeclarationSource,
      /expected_project_instance_id:\s*ProjectId,\s*expected_project_id:\s*ProjectId,\s*expected_revision:\s*u64,/u,
    )
    const nativeContractSource = nativeRequestStruct
      ? `${bindingDeclarationSource}\n${nativeFunctionSource}`
      : nativeFunctionSource
    assert.ok(
      occurrences(nativeContractSource, 'expected_project_instance_id') >= 2,
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

test('FOLD 3D timeline apply uses the current OCC runner and returned snapshot', () => {
  const launcherStart = app.indexOf('<Fold3dFramesLauncher')
  const launcherEnd = app.indexOf('/>', launcherStart)
  assert.ok(launcherStart >= 0 && launcherEnd > launcherStart, 'FOLD 3D launcher')
  const launcherProps = app.slice(launcherStart, launcherEnd + 2)
  assert.match(launcherProps, /runNativeEdit=\{runNativeEdit\}/u)
  assert.doesNotMatch(launcherProps, /onApplied|getProjectSnapshot/u)
  assert.match(
    fold3dFramesLauncher,
    /runNativeEdit:\s*Fold3dNativeEditRunner/u,
  )
  assert.match(
    fold3dFramesLauncher,
    /runNativeEdit\(\s*\(\s*projectId,\s*revision,\s*projectInstanceId\s*\)\s*=>\s*applyFold3dInstructionTimeline\(\s*projectId,\s*revision,\s*projectInstanceId,/u,
  )
  assert.doesNotMatch(fold3dFramesLauncher, /onApplied|getProjectSnapshot/u)

  const applyTimeline = typescriptFunctionSection(
    client,
    'applyFold3dInstructionTimeline',
  )
  assert.match(
    applyTimeline,
    /preview\.projectInstanceId !== expectedProjectInstanceId[\s\S]*preview\.projectId !== expectedProjectId[\s\S]*preview\.revision !== expectedRevision[\s\S]*invoke<ProjectSnapshot>\('apply_fold_3d_instruction_timeline'/u,
  )
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
        /\n(?:pub(?:\([^)]*\))?\s+)?(?:(?:async|const|unsafe)\s+|extern(?:\s+"[^"\r\n]+")?\s+)*fn ([a-z][a-z0-9_]*)(?:\s*<[^>{};\r\n]*>)?\s*\(/gu,
      ),
    ].map((match) => match[1]!))),
  ]
  const sections = new Map(productionFunctions.map((name) => [
    name,
    rustFunctionSectionFromSources(productions, name),
  ]))
  const productionFunctionSet = new Set(productionFunctions)
  const freeCalls = new Map(productionFunctions.map((name) => [
    name,
    new Set(rustFreeFunctionCalls(sections.get(name)!)),
  ]))
  const revisionChanging = new Set(productionFunctions.filter((name) => (
    ['execute_command', 'execute_undo', 'execute_redo'].some((candidate) => (
      freeCalls.get(name)!.has(candidate)
    ))
  )))
  const callersByCallee = new Map(
    productionFunctions.map((name) => [name, [] as string[]]),
  )
  for (const caller of productionFunctions) {
    for (const callee of freeCalls.get(caller)!) {
      if (callee !== caller && productionFunctionSet.has(callee)) {
        callersByCallee.get(callee)!.push(caller)
      }
    }
  }
  const queue = [...revisionChanging]
  for (let index = 0; index < queue.length; index += 1) {
    for (const caller of callersByCallee.get(queue[index]!) ?? []) {
      if (!revisionChanging.has(caller)) {
        revisionChanging.add(caller)
        queue.push(caller)
      }
    }
  }
  const commands = productions
    .flatMap((production) => [
      ...production.matchAll(
        /\n#\[tauri::command\]\n(?:#\[[^\n]+\]\n)*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn ([a-z][a-z0-9_]*)\(/gu,
      ),
    ].map((match) => match[1]!))
    .filter((name) => revisionChanging.has(name))
    .toSorted()
  assert.equal(new Set(commands).size, commands.length)
  return commands
}

function rustFreeFunctionCalls(text: string) {
  const result: string[] = []
  const calls = text.matchAll(
    /\b([a-z][a-z0-9_]*)(?:\s*::\s*<[^;{}\r\n]*>)?\s*\(/gu,
  )
  for (const call of calls) {
    let previous = call.index - 1
    while (previous >= 0 && /\s/u.test(text[previous]!)) previous -= 1
    if (text[previous] !== '.') result.push(call[1]!)
  }
  return result
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
    String.raw`\n(?:pub(?:\([^)]*\))?\s+)?(?:(?:async|const|unsafe)\s+|extern(?:\s+"[^"\r\n]+")?\s+)*fn ${name}(?:\s*<[^>{};\r\n]*>)?\s*\(`,
    'u',
  ).exec(rustStructuralCode(text))
  assert.ok(match, `${name} native function`)
  const start = match.index + 1
  const structuralCode = rustStructuralCode(text)
  const openingBrace = structuralCode.indexOf('{', start)
  assert.ok(openingBrace >= 0, `${name} opening brace`)
  let depth = 0
  for (let index = openingBrace; index < structuralCode.length; index += 1) {
    if (structuralCode[index] === '{') depth += 1
    if (structuralCode[index] === '}') {
      depth -= 1
      if (depth === 0) return text.slice(start, index + 1)
    }
  }
  assert.fail(`${name} closing brace`)
}

function rustFunctionSectionFromSources(texts: readonly string[], name: string) {
  const declaration = new RegExp(
    String.raw`\n(?:pub(?:\([^)]*\))?\s+)?(?:(?:async|const|unsafe)\s+|extern(?:\s+"[^"\r\n]+")?\s+)*fn ${name}(?:\s*<[^>{};\r\n]*>)?\s*\(`,
    'u',
  )
  const matches = texts.filter((text) => declaration.test(rustStructuralCode(text)))
  assert.equal(matches.length, 1, `${name} must have one native definition`)
  return rustFunctionSection(matches[0]!, name)
}

function rustStructSection(text: string, name: string) {
  const structuralCode = rustStructuralCode(text)
  const match = new RegExp(
    String.raw`\n(?:pub(?:\([^)]*\))?\s+)?struct ${name}(?:\s*<[^>{};\r\n]*>)?\s*\{`,
    'u',
  ).exec(structuralCode)
  assert.ok(match, `${name} native request struct`)
  const start = match.index + 1
  const openingBrace = structuralCode.indexOf('{', start)
  let depth = 0
  for (let index = openingBrace; index < structuralCode.length; index += 1) {
    if (structuralCode[index] === '{') depth += 1
    if (structuralCode[index] === '}') {
      depth -= 1
      if (depth === 0) return text.slice(start, index + 1)
    }
  }
  assert.fail(`${name} closing brace`)
}

function rustStructSectionFromSources(texts: readonly string[], name: string) {
  const declaration = new RegExp(
    String.raw`\n(?:pub(?:\([^)]*\))?\s+)?struct ${name}(?:\s*<[^>{};\r\n]*>)?\s*\{`,
    'u',
  )
  const matches = texts.filter((text) => declaration.test(rustStructuralCode(text)))
  assert.equal(matches.length, 1, `${name} must have one native definition`)
  return rustStructSection(matches[0]!, name)
}

function rustProductionSection(text: string) {
  const structuralCode = rustStructuralCode(text)
  const result = structuralCode.split('')
  for (const [start, end] of rustCfgTestInlineModuleRanges(
    text,
    structuralCode,
  )) {
    for (let index = start; index < end; index += 1) {
      if (result[index] !== '\r' && result[index] !== '\n') {
        result[index] = ' '
      }
    }
  }
  return result.join('')
}

function rustCodeWithoutLineComments(text: string) {
  return text.replace(/\/\/[^\r\n]*/gu, '')
}

function rustSources(directory: URL): Array<readonly [string, string]> {
  return productionRustSources(allRustSources(directory))
}

function allRustSources(directory: URL): Array<readonly [string, string]> {
  const result: Array<readonly [string, string]> = []
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const child = new URL(`${entry.name}${entry.isDirectory() ? '/' : ''}`, directory)
    if (entry.isDirectory()) {
      result.push(...allRustSources(child))
    } else if (entry.isFile() && entry.name.endsWith('.rs')) {
      result.push([child.pathname, readFileSync(child, 'utf8')])
    }
  }
  return result
}

function productionRustSources(
  sources: ReadonlyArray<readonly [string, string]>,
) {
  const normalizedSources = sources.map(([path, text]) => (
    [path.replaceAll('\\', '/'), text] as const
  ))
  const sourcePaths = new Set(normalizedSources.map(([path]) => path))
  const edgesBySource = new Map(normalizedSources.map(([sourcePath, text]) => [
    sourcePath,
    rustSourceEdges(sourcePath, text, sourcePaths),
  ]))
  const incomingPaths = new Set(
    [...edgesBySource.values()].flatMap((edges) => (
      edges.map(({ target }) => target)
    )),
  )
  const productionReachable = new Set(
    [...sourcePaths].filter((path) => !incomingPaths.has(path)),
  )
  const testReachable = new Set<string>()
  const propagate = () => {
    const queue = [...sourcePaths].filter((path) => (
      productionReachable.has(path) || testReachable.has(path)
    ))
    const queued = new Set(queue)
    for (let index = 0; index < queue.length; index += 1) {
      const sourcePath = queue[index]!
      queued.delete(sourcePath)
      const sourceIsProduction = productionReachable.has(sourcePath)
      const sourceIsTest = testReachable.has(sourcePath)
      for (const edge of edgesBySource.get(sourcePath) ?? []) {
        let changed = false
        if (sourceIsProduction && !edge.testOnly) {
          const size = productionReachable.size
          productionReachable.add(edge.target)
          changed ||= productionReachable.size !== size
        }
        if (sourceIsTest || (sourceIsProduction && edge.testOnly)) {
          const size = testReachable.size
          testReachable.add(edge.target)
          changed ||= testReachable.size !== size
        }
        if (changed && !queued.has(edge.target)) {
          queue.push(edge.target)
          queued.add(edge.target)
        }
      }
    }
  }
  propagate()
  for (const path of sourcePaths) {
    if (!productionReachable.has(path) && !testReachable.has(path)) {
      productionReachable.add(path)
    }
  }
  propagate()
  return normalizedSources.filter(([path]) => productionReachable.has(path))
}

function rustSourceEdges(
  sourcePath: string,
  text: string,
  sourcePaths: ReadonlySet<string>,
) {
  const structuralCode = rustStructuralCode(text)
  const testRanges = rustCfgTestInlineModuleRanges(text, structuralCode)
  const edges: Array<{ target: string, testOnly: boolean }> = []
  for (const declaration of rustExternalModuleDeclarations(text, structuralCode)) {
    const declaredInsideTestModule = offsetIsInRanges(
      declaration.index,
      testRanges,
    )
    if (declaredInsideTestModule) continue
    const target = rustModuleSourcePath(
      sourcePath,
      declaration.name,
      declaration.path,
      sourcePaths,
    )
    if (target !== undefined) {
      edges.push({
        target,
        testOnly: declaration.cfgTest || declaredInsideTestModule,
      })
    }
  }
  for (const include of rustIncludeDeclarations(
    sourcePath,
    text,
    structuralCode,
    testRanges,
  )) {
    if (sourcePaths.has(include.target)) edges.push(include)
  }
  return edges
}

function rustExternalModuleDeclarations(
  text: string,
  structuralCode = rustStructuralCode(text),
) {
  return [
    ...structuralCode.matchAll(
      /((?:^[ \t]*#\[[^\r\n]+\][ \t]*\r?\n)*)^[ \t]*(?:pub(?:\([^)\r\n]*\))?[ \t]+)?mod[ \t]+([a-zA-Z_][a-zA-Z0-9_]*)[ \t]*;/gmu,
    ),
  ].map((match) => {
    const attributes = rustAttributes(
      text,
      structuralCode,
      match.index,
      match[1]!.length,
    )
    return {
      ...attributes,
      index: match.index,
      name: match[2]!,
    }
  })
}

function rustAttributes(
  text: string,
  structuralCode: string,
  start: number,
  length: number,
) {
  const structuralAttributes = structuralCode.slice(start, start + length)
  let cfgTest = false
  let path: string | undefined
  for (const attribute of structuralAttributes.matchAll(
    /^[ \t]*#\[[^\r\n]+\][ \t]*$/gmu,
  )) {
    const structuralAttribute = attribute[0]
    if (
      /^[ \t]*#\[\s*cfg\s*\(\s*test\s*\)\s*\][ \t]*$/u.test(
        structuralAttribute,
      )
    ) {
      cfgTest = true
    }
    if (/^[ \t]*#\[\s*path\s*=/u.test(structuralAttribute)) {
      const attributeStart = start + attribute.index
      const originalAttribute = text.slice(
        attributeStart,
        attributeStart + structuralAttribute.length,
      )
      const parsedPath = /^[ \t]*#\[\s*path\s*=\s*"([^"\r\n]+)"\s*\][ \t]*$/u.exec(
        originalAttribute,
      )
      assert.ok(parsedPath, 'Rust path attributes must use one string literal')
      assert.equal(path, undefined, 'a Rust module must have at most one path attribute')
      path = parsedPath[1]!
    }
    if (
      /^[ \t]*#\[\s*cfg_attr\s*\(/u.test(structuralAttribute)
      && /\bpath\s*=/u.test(structuralAttribute)
    ) {
      assert.fail('conditional Rust path attributes are unsupported')
    }
  }
  return { cfgTest, path }
}

function rustIncludeDeclarations(
  sourcePath: string,
  text: string,
  structuralCode: string,
  testRanges: ReadonlyArray<readonly [number, number]>,
) {
  const result: Array<{ target: string, testOnly: boolean }> = []
  const includes = structuralCode.matchAll(
    /((?:^[ \t]*#\[[^\r\n]+\][ \t]*\r?\n)*)^[ \t]*include\s*!\s*\(/gmu,
  )
  for (const include of includes) {
    const includeOffset = include[0].lastIndexOf('include')
    const includeStart = include.index + includeOffset
    const literal = /^include\s*!\s*\(\s*"([^"\r\n]+)"\s*\)\s*;/u.exec(
      text.slice(includeStart),
    )
    assert.ok(literal, 'Rust include! edges must use one string literal')
    const attributes = rustAttributes(
      text,
      structuralCode,
      include.index,
      include[1]!.length,
    )
    result.push({
      target: new URL(literal[1]!, `file://${sourcePath}`)
        .pathname
        .replaceAll('\\', '/'),
      testOnly: attributes.cfgTest || offsetIsInRanges(
        include.index,
        testRanges,
      ),
    })
  }
  return result
}

function offsetIsInRanges(
  offset: number,
  ranges: ReadonlyArray<readonly [number, number]>,
) {
  return ranges.some(([start, end]) => start <= offset && offset < end)
}

function rustModuleSourcePath(
  sourcePath: string,
  moduleName: string,
  explicitPath: string | undefined,
  sourcePaths: ReadonlySet<string>,
) {
  const slash = sourcePath.lastIndexOf('/')
  const directory = sourcePath.slice(0, slash + 1)
  const fileName = sourcePath.slice(slash + 1)
  const stem = fileName.endsWith('.rs') ? fileName.slice(0, -3) : fileName
  const moduleDirectory = (
    stem === 'lib' || stem === 'main' || stem === 'mod'
      ? directory
      : `${directory}${stem}/`
  )
  const candidates = explicitPath === undefined
    ? [
        `${moduleDirectory}${moduleName}.rs`,
        `${moduleDirectory}${moduleName}/mod.rs`,
      ]
    : [
        new URL(explicitPath, `file://${sourcePath}`).pathname,
      ]
  const matches = candidates
    .map((path) => path.replaceAll('\\', '/'))
    .filter((path) => sourcePaths.has(path))
  assert.ok(
    matches.length <= 1,
    `${sourcePath} module ${moduleName} must resolve to at most one source`,
  )
  return matches[0]
}

function rustCfgTestInlineModuleRanges(text: string, structuralCode: string) {
  const ranges: Array<readonly [number, number]> = []
  const modules = structuralCode.matchAll(
    /((?:^[ \t]*#\[[^\r\n]+\][ \t]*\r?\n)*)^[ \t]*(?:pub(?:\([^)\r\n]*\))?[ \t]+)?mod[ \t]+[a-zA-Z_][a-zA-Z0-9_]*[ \t]*\{/gmu,
  )
  for (const module of modules) {
    const attributes = rustAttributes(
      text,
      structuralCode,
      module.index,
      module[1]!.length,
    )
    if (!attributes.cfgTest) continue
    const openingBrace = module.index + module[0].lastIndexOf('{')
    let depth = 0
    for (let index = openingBrace; index < structuralCode.length; index += 1) {
      if (structuralCode[index] === '{') depth += 1
      if (structuralCode[index] === '}') {
        depth -= 1
        if (depth === 0) {
          ranges.push([openingBrace + 1, index])
          break
        }
      }
    }
  }
  return ranges
}

function rustStructuralCode(text: string) {
  const cached = rustStructuralCodeCache.get(text)
  if (cached !== undefined) return cached
  const result = text.split('')
  const characterLiteral = /'(?:\\(?:x[0-9a-fA-F]{2}|u\{[0-9a-fA-F_]+\}|.)|[^\\'\r\n])'/uy
  const mask = (start: number, end: number) => {
    for (let index = start; index < end; index += 1) {
      if (result[index] !== '\r' && result[index] !== '\n') {
        result[index] = ' '
      }
    }
  }
  for (let index = 0; index < text.length;) {
    if (text.startsWith('//', index)) {
      const end = text.indexOf('\n', index + 2)
      const next = end < 0 ? text.length : end
      mask(index, next)
      index = next
      continue
    }
    if (text.startsWith('/*', index)) {
      let depth = 1
      let end = index + 2
      while (end < text.length && depth > 0) {
        if (text.startsWith('/*', end)) {
          depth += 1
          end += 2
        } else if (text.startsWith('*/', end)) {
          depth -= 1
          end += 2
        } else {
          end += 1
        }
      }
      mask(index, end)
      index = end
      continue
    }
    let rawDelimiterStart = -1
    if (text[index] === 'r') {
      rawDelimiterStart = index + 1
    } else if (text[index] === 'b' && text[index + 1] === 'r') {
      rawDelimiterStart = index + 2
    }
    if (rawDelimiterStart >= 0) {
      let quote = rawDelimiterStart
      while (text[quote] === '#') quote += 1
      if (text[quote] !== '"') {
        index += 1
        continue
      }
      const hashes = text.slice(rawDelimiterStart, quote)
      const closing = `"${hashes}`
      const contentStart = quote + 1
      const closingStart = text.indexOf(closing, contentStart)
      const end = closingStart < 0
        ? text.length
        : closingStart + closing.length
      mask(index, end)
      index = end
      continue
    }
    if (text[index] === '"') {
      let end = index + 1
      while (end < text.length) {
        if (text[end] === '\\') {
          end += 2
        } else if (text[end] === '"') {
          end += 1
          break
        } else {
          end += 1
        }
      }
      mask(index, end)
      index = end
      continue
    }
    if (text[index] === "'") {
      characterLiteral.lastIndex = index
      const character = characterLiteral.exec(text)
      if (character !== null) {
        const end = index + character[0].length
        mask(index, end)
        index = end
        continue
      }
    }
    index += 1
  }
  const structuralCode = result.join('')
  rustStructuralCodeCache.set(text, structuralCode)
  return structuralCode
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
