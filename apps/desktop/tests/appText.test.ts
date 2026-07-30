import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import ts from 'typescript'

import { APP_TEXT as TEXT } from '../src/lib/appText.ts'
import { formatLocalizedText } from '../src/lib/i18n.ts'

test('app text catalog is closed and deeply frozen', () => {
  assert.equal(Object.keys(TEXT).length, 891)
  assert.equal(Object.isFrozen(TEXT), true)

  for (const entry of Object.values(TEXT)) {
    assert.deepEqual(Object.keys(entry), ['ja', 'en'])
    assert.equal(Object.isFrozen(entry), true)
    assert.equal(typeof entry.ja, 'string')
    assert.equal(typeof entry.en, 'string')
  }

  assert.deepEqual(TEXT.grid, { ja: 'グリッド', en: 'Grid' })
  assert.deepEqual(TEXT.projectActions, {
    ja: 'プロジェクト操作',
    en: 'Project actions',
  })
  assert.equal(
    formatLocalizedText(
      'en',
      TEXT.createdNameASaveLocationHasNotBeenSetYet,
      { name: 'Crane' },
    ),
    'Created “Crane”. A save location has not been set yet.',
  )
})

test('App and its extracted helpers reference the catalog for every fixed localized object', () => {
  const paths = [
    new URL('../src/App.tsx', import.meta.url),
    new URL('../src/lib/appGeometry.ts', import.meta.url),
    new URL('../src/lib/appNumericExpression.ts', import.meta.url),
    new URL('../src/lib/appPresentation.ts', import.meta.url),
    new URL('../src/lib/snapInspectorOptions.ts', import.meta.url),
    new URL('../src/lib/useFoldImportWorkflow.ts', import.meta.url),
    new URL('../src/lib/useSvgImportWorkflow.ts', import.meta.url),
    new URL('../src/lib/importWorkflowSupport.ts', import.meta.url),
    new URL('../src/components/BeginnerCandidateControls.tsx', import.meta.url),
    new URL('../src/components/BeginnerCandidateResults.tsx', import.meta.url),
    new URL('../src/components/BeginnerGeneratedInstructionList.tsx', import.meta.url),
    new URL('../src/components/BeginnerRecognitionPanel.tsx', import.meta.url),
    new URL('../src/components/PaperInspectorSection.tsx', import.meta.url),
    new URL('../src/components/HistoryLimitInspectorSection.tsx', import.meta.url),
    new URL('../src/components/FoldTechniqueInspectorSection.tsx', import.meta.url),
    new URL('../src/components/SnapInspectorSection.tsx', import.meta.url),
    new URL('../src/components/MirrorSelectionPanel.tsx', import.meta.url),
    new URL('../src/components/ElementMetadataForm.tsx', import.meta.url),
    new URL('../src/components/SelectedLineInspector.tsx', import.meta.url),
    new URL('../src/components/SelectedFaceInspector.tsx', import.meta.url),
    new URL('../src/components/SelectedVertexInspector.tsx', import.meta.url),
    new URL('../src/components/BeginnerDesignEditorSection.tsx', import.meta.url),
    new URL('../src/components/BeginnerDesignSources.tsx', import.meta.url),
    new URL('../src/components/BeginnerReferenceAssetPanel.tsx', import.meta.url),
    new URL('../src/components/BeginnerReferenceSuggestionPanel.tsx', import.meta.url),
    new URL('../src/components/BeginnerDesignConstraints.tsx', import.meta.url),
    new URL('../src/components/BeginnerSkeletonEditor.tsx', import.meta.url),
    new URL('../src/components/BeginnerProtrusionEditor.tsx', import.meta.url),
    new URL('../src/components/ValidationInspectorSections.tsx', import.meta.url),
    new URL('../src/components/ProjectMemoAndCandidateSection.tsx', import.meta.url),
  ]
  const sources = paths.map((path) => ({
    path,
    source: readFileSync(path, 'utf8'),
  }))
  let inlineFixedLocalizedObjects = 0

  const visit = (node: ts.Node): void => {
    if (ts.isObjectLiteralExpression(node)) {
      const fixedLocales = new Set<string>()
      let fixedOnly = true
      for (const property of node.properties) {
        if (
          !ts.isPropertyAssignment(property)
          || !(
            ts.isIdentifier(property.name)
            || ts.isStringLiteral(property.name)
          )
          || !(
            ts.isStringLiteral(property.initializer)
            || ts.isNoSubstitutionTemplateLiteral(property.initializer)
          )
        ) {
          fixedOnly = false
          break
        }
        fixedLocales.add(property.name.text)
      }
      if (
        fixedOnly
        && fixedLocales.size === 2
        && fixedLocales.has('ja')
        && fixedLocales.has('en')
      ) {
        inlineFixedLocalizedObjects += 1
      }
    }
    ts.forEachChild(node, visit)
  }
  for (const { path, source } of sources) {
    visit(ts.createSourceFile(
      path.pathname,
      source,
      ts.ScriptTarget.Latest,
      true,
      path.pathname.endsWith('.tsx') ? ts.ScriptKind.TSX : ts.ScriptKind.TS,
    ))
  }

  assert.equal(inlineFixedLocalizedObjects, 0)
  assert.equal(
    sources.reduce(
      (count, { source }) => (
        count + (source.match(/APP_TEXT\.[A-Za-z_$][A-Za-z0-9_$]*/gu)?.length ?? 0)
      ),
      0,
    ),
    968,
  )
})
