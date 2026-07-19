import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const cssSource = readFileSync(
  new URL('../src/App.css', import.meta.url),
  'utf8',
)
const previewSource = readFileSync(
  new URL('../src/components/FoldPreview.tsx', import.meta.url),
  'utf8',
)

test('every blocking collision badge uses the shared danger-color contract', () => {
  assert.match(
    cssSource,
    /\.fold-preview-collision\[data-collision-risk="blocking"\]\s*\{/u,
  )
})

test('the collision alert is not hidden with the visual status stack', () => {
  assert.doesNotMatch(
    previewSource,
    /className="fold-preview-status-stack"\s+aria-hidden="true"/u,
  )
  assert.match(
    previewSource,
    /className=\{`fold-preview-motion \$\{motionBadgeClass\}`\}[\s\S]*?aria-hidden="true"/u,
  )
  assert.match(
    previewSource,
    /className=\{`fold-preview-correction \$\{correctionAnalysisView\.badgeClass\}`\}[\s\S]*?aria-hidden="true"/u,
  )
})
