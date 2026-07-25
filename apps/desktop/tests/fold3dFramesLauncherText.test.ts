import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  FOLD3D_FRAMES_LAUNCHER_TEXT as TEXT,
} from '../src/lib/fold3dFramesLauncherText.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
} from '../src/lib/i18n.ts'

const KEYS = [
  'openError',
  'timelineError',
  'selectionError',
  'poseError',
  'launcher',
  'title',
  'close',
  'readOnlyExplanation',
  'frame',
  'frameOption',
  'framePreviewAlt',
  'compatiblePose',
  'incompatiblePose',
  'confirmPoseReplacement',
  'poseHistoryExplanation',
  'poseApplied',
  'applyPose',
  'timelineTitle',
  'timelineSummary',
  'confirmTimeline',
  'applyTimeline',
] as const

test('FOLD 3D frame launcher catalog is closed and deeply frozen', () => {
  assert.deepEqual(Object.keys(TEXT), KEYS)
  assert.equal(Object.isFrozen(TEXT), true)
  for (const key of KEYS) {
    assert.deepEqual(Object.keys(TEXT[key]), ['ja', 'en'], key)
    assert.equal(Object.isFrozen(TEXT[key]), true, key)
  }
  assert.equal(
    selectLocalizedText('ja', TEXT.launcher),
    'FOLD 3Dフレームをプレビュー',
  )
  assert.equal(
    selectLocalizedText('en', TEXT.timelineError),
    'The project changed or these frames are not one compatible linear chain.',
  )
})

test('FOLD 3D frame launcher placeholders are locale-equivalent', () => {
  assert.deepEqual(placeholderMap(TEXT), {
    frameOption: {
      ja: ['index', 'vertexCount'],
      en: ['index', 'vertexCount'],
    },
    framePreviewAlt: { ja: ['index'], en: ['index'] },
    compatiblePose: { ja: ['hingeCount'], en: ['hingeCount'] },
    timelineSummary: { ja: ['frameCount'], en: ['frameCount'] },
  })
  assert.equal(
    formatLocalizedText('ja', TEXT.frameOption, {
      index: 2,
      vertexCount: 7,
    }),
    'フレーム 2・頂点 7',
  )
  assert.equal(
    formatLocalizedText('en', TEXT.timelineSummary, { frameCount: 3 }),
    '3 complete poses will be appended atomically at 1.0 second each. Geometry is unchanged; Undo/Redo treats this as one history entry.',
  )
})

test('FOLD 3D frame launcher keeps fixed display copy in the catalog', () => {
  const source = readFileSync(
    new URL(
      '../src/components/Fold3dFramesLauncher.tsx',
      import.meta.url,
    ),
    'utf8',
  )
  assert.match(source, /FOLD3D_FRAMES_LAUNCHER_TEXT as TEXT/u)
  assert.match(source, /useState<ErrorTextKey \| null>\(null\)/u)
  assert.match(source, /TEXT\[error\]/u)
  assert.doesNotMatch(source, /[ぁ-んァ-ン一-龯]/u)
  assert.doesNotMatch(source, /\blocale\.startsWith\(/u)
  assert.doesNotMatch(source, /\ben\s*\?/u)
})

function placeholderMap(
  value: Readonly<Record<string, Readonly<Record<'ja' | 'en', string>>>>,
) {
  return Object.fromEntries(
    Object.entries(value).flatMap(([key, localized]) => {
      const ja = placeholders(localized.ja)
      const en = placeholders(localized.en)
      return ja.length === 0 && en.length === 0
        ? []
        : [[key, { ja, en }]]
    }),
  )
}

function placeholders(value: string) {
  return [...value.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\}/gu)]
    .map((match) => match[1])
}
