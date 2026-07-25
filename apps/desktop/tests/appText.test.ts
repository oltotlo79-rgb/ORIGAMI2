import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import ts from 'typescript'

import { APP_TEXT as TEXT } from '../src/lib/appText.ts'
import { formatLocalizedText } from '../src/lib/i18n.ts'

test('app text catalog is closed and deeply frozen', () => {
  assert.equal(Object.keys(TEXT).length, 861)
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

test('App references the catalog for every fixed localized object', () => {
  const path = new URL('../src/App.tsx', import.meta.url)
  const source = readFileSync(path, 'utf8')
  const sourceFile = ts.createSourceFile(
    path.pathname,
    source,
    ts.ScriptTarget.Latest,
    true,
    ts.ScriptKind.TSX,
  )
  let inlineFixedLocalizedObjects = 0

  const visit = (node: ts.Node) => {
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
  visit(sourceFile)

  assert.equal(inlineFixedLocalizedObjects, 0)
  assert.equal(
    source.match(/APP_TEXT\.[A-Za-z_$][A-Za-z0-9_$]*/gu)?.length,
    935,
  )
})
