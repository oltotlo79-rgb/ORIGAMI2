import assert from 'node:assert/strict'
import { readFileSync, readdirSync } from 'node:fs'
import { join, resolve } from 'node:path'
import test from 'node:test'

const root = resolve(import.meta.dirname, '..')
const componentDirectory = join(root, 'src', 'components')

test('production components do not use fixed literal ARIA labels', () => {
  for (const name of readdirSync(componentDirectory).filter((value) => value.endsWith('.tsx'))) {
    const source = readFileSync(join(componentDirectory, name), 'utf8')
    assert.doesNotMatch(source, /aria-label\s*=\s*["'][^"']+["']/u, name)
  }
})

test('major new features retain reviewed Japanese and English UI contracts', () => {
  const app = [
    readFileSync(join(root, 'src', 'App.tsx'), 'utf8'),
    readFileSync(join(root, 'src', 'lib', 'appText.ts'), 'utf8'),
  ].join('\n')
  assert.match(
    app,
    /evaluateTop3Of27Designs: localized\('27案から上位3案を評価', 'Evaluate top 3 of 27 designs'\)/u,
  )
  assert.match(
    app,
    /top3FromThe27DesignSearch: localized\('27案探索の上位3案', 'Top 3 from the 27-design search'\)/u,
  )
  assert.match(
    app,
    /aGLB20ModelIsAReadOnlyVisual: localized\(\s*'GLB 2\.0モデルは読み取り専用の視覚参照です。[^']*'/u,
  )

  const stacked = readFileSync(join(componentDirectory, 'StackedFoldPanel.tsx'), 'utf8')
  assert.match(stacked, /t\('スケジュール証明', 'Schedule certificate'\)/u)
  assert.match(stacked, /t\('衝突証明', 'Collision certificate'\)/u)
  assert.match(stacked, /t\('閉路証明', 'Closure certificate'\)/u)
  assert.doesNotMatch(stacked, /<dt>(?:schedule|collision|closure)<\/dt>/u)
  assert.doesNotMatch(stacked, /<dd>\{view\.response\.continuousPath\.continuousCertificateModelId/u)
  assert.doesNotMatch(stacked, /<title>\{`[^`]*\$\{face\}/u)

  const release = readFileSync(
    join(root, 'src', 'lib', 'updateCheckControlText.ts'),
    'utf8',
  )
  for (const key of ['popoverSummary', 'title', 'enabled', 'checkButton', 'openRelease']) {
    assert.match(release, new RegExp(`${key}: Object\\.freeze\\(\\{[\\s\\S]*?ja: '[^']+'[\\s\\S]*?en: '[^']+'`, 'u'))
  }
})
