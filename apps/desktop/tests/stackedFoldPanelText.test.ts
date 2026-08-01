import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import ts from 'typescript'

import { formatLocalizedText } from '../src/lib/i18n.ts'
import {
  STACKED_FOLD_PANEL_TEXT as TEXT,
} from '../src/lib/stackedFoldPanelText.ts'

test('stacked-fold panel catalog is complete and deeply frozen', () => {
  assert.equal(Object.keys(TEXT).length, 164)
  assert.equal(Object.isFrozen(TEXT), true)

  for (const entry of Object.values(TEXT)) {
    assert.deepEqual(Object.keys(entry), ['ja', 'en'])
    assert.equal(Object.isFrozen(entry), true)
  }

  assert.deepEqual(TEXT.scheduleCertificate, {
    ja: 'スケジュール証明',
    en: 'Schedule certificate',
  })
  assert.deepEqual(TEXT.collisionCertificate, {
    ja: '衝突証明',
    en: 'Collision certificate',
  })
  assert.deepEqual(TEXT.closureCertificate, {
    ja: '閉路証明',
    en: 'Closure certificate',
  })
})

test('stacked-fold formatted copy keeps equivalent bounded placeholders', () => {
  const placeholders = Object.fromEntries(
    Object.entries(TEXT).flatMap(([key, entry]) => {
      const ja = [...entry.ja.matchAll(/\{([^}]+)\}/gu)]
        .map((match) => match[1])
      const en = [...entry.en.matchAll(/\{([^}]+)\}/gu)]
        .map((match) => match[1])
      return ja.length === 0 && en.length === 0 ? [] : [[key, { ja, en }]]
    }),
  )
  assert.deepEqual(placeholders, {
    savedCompilerProvenance: {
      ja: ['kind', 'count'],
      en: ['kind', 'count'],
    },
    boundedSchedule: { ja: ['count'], en: ['count'] },
    cyclePathProgress: {
      ja: ['states', 'stateLimit', 'transitions', 'transitionLimit'],
      en: ['states', 'stateLimit', 'transitions', 'transitionLimit'],
    },
    persistedLayerPairsOmitted: {
      ja: ['visible', 'remaining'],
      en: ['visible', 'remaining'],
    },
    searchPathProgress: {
      ja: ['states', 'stateLimit', 'transitions', 'transitionLimit'],
      en: ['states', 'stateLimit', 'transitions', 'transitionLimit'],
    },
    certifiedPathTransitionCount: { ja: ['count'], en: ['count'] },
    transitionIndex: { ja: ['index'], en: ['index'] },
    namedTechniqueWillBeSaved: { ja: ['name'], en: ['name'] },
    backBottomFaceIndex: { ja: ['index'], en: ['index'] },
    frontTopFaceIndex: { ja: ['index'], en: ['index'] },
    middleLayerFaceIndex: { ja: ['index'], en: ['index'] },
  })
  assert.equal(
    formatLocalizedText('en', TEXT.cyclePathProgress, {
      states: 3,
      stateLimit: 32,
      transitions: 7,
      transitionLimit: 64,
    }),
    'Cycle states 3/32; transitions 7/64',
  )
  assert.equal(
    formatLocalizedText('ja', TEXT.namedTechniqueWillBeSaved, {
      name: '中割り',
    }),
    '名前付き技法「中割り」として認証済み姿勢を手順へ保存します。PDF/SVG折り図にも同じ手順が使われます。',
  )
})

test('stacked-fold components have no inline localized pair left', () => {
  const paths = [
    '../src/components/StackedFoldPanel.tsx',
    '../src/components/LayerOrderViewer.tsx',
  ].map((path) => new URL(path, import.meta.url))
  const source = paths
    .map((path) => readFileSync(path, 'utf8'))
    .join('\n')
  const sourceFile = ts.createSourceFile(
    'stacked-fold-components.tsx',
    source,
    ts.ScriptTarget.Latest,
    true,
    ts.ScriptKind.TSX,
  )
  let localizedPairObjects = 0

  const visit = (node: ts.Node) => {
    if (ts.isObjectLiteralExpression(node)) {
      const names = node.properties.flatMap((property) =>
        ts.isPropertyAssignment(property)
        && (
          ts.isIdentifier(property.name)
          || ts.isStringLiteral(property.name)
        )
          ? [property.name.text]
          : [],
      )
      if (
        node.properties.length === 2
        && names.length === 2
        && names.includes('ja')
        && names.includes('en')
      ) {
        localizedPairObjects += 1
      }
    }
    ts.forEachChild(node, visit)
  }
  visit(sourceFile)

  assert.equal(localizedPairObjects, 0)
  assert.doesNotMatch(source, /\bconst t\s*=/u)
  assert.equal(
    source.match(/TEXT\.[A-Za-z_$][A-Za-z0-9_$]*/gu)?.length,
    183,
  )
})
