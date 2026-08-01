import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const APP_PATH = '../src/App.tsx'
const EXTRACTED_MODULES = [
  '../src/components/BeginnerDesignConstraints.tsx',
  '../src/components/BeginnerDesignEditorSection.tsx',
  '../src/components/BeginnerDesignSources.tsx',
  '../src/components/BeginnerProtrusionEditor.tsx',
  '../src/components/BeginnerReferenceAssetPanel.tsx',
  '../src/components/BeginnerReferenceSuggestionPanel.tsx',
  '../src/components/BeginnerSkeletonEditor.tsx',
  '../src/components/ElementMetadataForm.tsx',
  '../src/components/FoldTechniqueInspectorSection.tsx',
  '../src/components/HistoryLimitInspectorSection.tsx',
  '../src/components/LayerOrderViewer.tsx',
  '../src/components/MirrorSelectionPanel.tsx',
  '../src/components/PaperInspectorSection.tsx',
  '../src/components/ProjectMemoAndCandidateSection.tsx',
  '../src/components/SelectedFaceInspector.tsx',
  '../src/components/SelectedLineInspector.tsx',
  '../src/components/SelectedVertexInspector.tsx',
  '../src/components/SnapInspectorSection.tsx',
  '../src/components/ValidationInspectorSections.tsx',
  '../src/lib/beginnerGeneratedPlanContract.ts',
  '../src/lib/beginnerCandidateScoreContract.ts',
  '../src/lib/beginnerGeneratedPlanInstructionContract.ts',
  '../src/lib/beginnerGeneratedPlanSnapshot.ts',
  '../src/lib/beginnerGeneratedPlanTopologyContract.ts',
  '../src/lib/beginnerGeneratedPlanTypes.ts',
  '../src/lib/beginnerGenericFeatureBindingContract.ts',
  '../src/lib/beginnerProtrusionKinds.ts',
  '../src/lib/sha256Bytes.ts',
  '../src/lib/snapInspectorOptions.ts',
] as const

test('App remains below the audited 6,000 physical-line boundary', () => {
  assert.ok(physicalLineCount(source(APP_PATH)) < 6_000)
})

test('each extracted responsibility remains below 500 physical lines', () => {
  for (const path of EXTRACTED_MODULES) {
    assert.ok(
      physicalLineCount(source(path)) < 500,
      `${path} must remain below 500 physical lines`,
    )
  }
})

test('the shared layer-order viewer stays decoupled from the stacked-fold panel', () => {
  const globalPanel = source(
    '../src/components/GlobalFlatFoldabilityPanel.tsx',
  )
  const stackedPanel = source(
    '../src/components/StackedFoldPanel.tsx',
  )
  assert.match(
    globalPanel,
    /from '\.\/LayerOrderViewer\.tsx'/u,
  )
  assert.doesNotMatch(globalPanel, /from '\.\/StackedFoldPanel\.tsx'/u)
  assert.match(stackedPanel, /export \{ LayerOrderViewer \}/u)
})

test('extraction boundaries remain on natural JSX lines', () => {
  for (const path of [APP_PATH, ...EXTRACTED_MODULES]) {
    assert.doesNotMatch(
      source(path),
      /(?:\/>|<\/section>|\)\}) {2,}(?:<|\{)/u,
      `${path} contains adjacent JSX boundaries joined by padding`,
    )
  }
})

function source(relativePath: string) {
  return readFileSync(new URL(relativePath, import.meta.url), 'utf8')
}

function physicalLineCount(value: string) {
  const lines = value.split(/\r?\n/u)
  return value.endsWith('\n') ? lines.length - 1 : lines.length
}
