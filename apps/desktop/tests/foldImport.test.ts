import assert from 'node:assert/strict'
import test from 'node:test'
import {
  foldBoundaryCandidate,
  foldBoundaryCandidateLabel,
  foldBoundaryPreviewEdgeSet,
  foldAssignmentLabel,
  foldImportPreviewFileName,
  foldImportSuggestedName,
  foldImportTargetLabel,
  foldImportTargetOptions,
  foldImportWarningMessage,
  foldPreviewBounds,
  initialFoldBoundaryCandidateId,
  initialFoldImportMapping,
  isAllowedFoldImportTarget,
  isValidFoldImportName,
  parseFoldImportScale,
  unresolvedFoldAssignments,
  type FoldImportAssignmentSummary,
  type FoldImportPreview,
} from '../src/lib/foldImport.ts'

const summaries: FoldImportAssignmentSummary[] = [
  { assignment: 'B', count: 4 },
  { assignment: 'M', count: 2 },
  { assignment: 'V', count: 1 },
  { assignment: 'F', count: 3 },
  { assignment: 'U', count: 1 },
  { assignment: 'C', count: 2 },
  { assignment: 'J', count: 1 },
]

test('FOLD import starts only semantically direct assignments with defaults', () => {
  assert.deepEqual(initialFoldImportMapping(summaries), {
    M: 'mountain',
    V: 'valley',
    C: 'cut',
  })
  assert.deepEqual(
    unresolvedFoldAssignments(summaries, initialFoldImportMapping(summaries)),
    ['F', 'U', 'J'],
  )
})

test('FOLD assignment targets preserve direct meanings and constrain lossy choices', () => {
  assert.deepEqual(foldImportTargetOptions('M').map(({ value }) => value), ['mountain'])
  assert.deepEqual(foldImportTargetOptions('V').map(({ value }) => value), ['valley'])
  assert.deepEqual(foldImportTargetOptions('F').map(({ value }) => value), [
    'auxiliary',
    'ignore',
  ])
  assert.deepEqual(foldImportTargetOptions('U').map(({ value }) => value), [
    'mountain',
    'valley',
    'auxiliary',
    'ignore',
  ])
  assert.deepEqual(foldImportTargetOptions('C').map(({ value }) => value), ['cut', 'ignore'])
  assert.deepEqual(foldImportTargetOptions('J').map(({ value }) => value), [
    'auxiliary',
    'ignore',
  ])
  assert.equal(isAllowedFoldImportTarget('F', 'mountain'), false)
  assert.equal(isAllowedFoldImportTarget('U', 'mountain'), true)
  assert.equal(foldAssignmentLabel('B'), 'B · 用紙境界')
  assert.equal(foldAssignmentLabel('B', 'en'), 'B · Paper boundary')
  assert.equal(foldAssignmentLabel('J', 'en'), 'J · Face join')
  assert.equal(foldImportTargetLabel('auxiliary'), '補助線')
  assert.equal(foldImportTargetLabel('auxiliary', 'en'), 'Auxiliary line')
  assert.equal(foldImportTargetLabel('ignore', 'en'), 'Do not import')
})

test('native FOLD warnings and fallback names localize without exposing unknown text', () => {
  assert.equal(
    foldImportWarningMessage(
      '辺の割当情報（edges_assignment）がないため、折り線種を確認・指定してください。',
      'en',
    ),
    'The optional edges_assignment array is missing. Review the paper boundary and explicitly map every remaining unassigned line.',
  )
  assert.equal(
    foldImportWarningMessage(
      '外周を一意に確定できないため、取り込む用紙外周を選択してください。',
      'en',
    ),
    'The source assignments do not establish one valid paper boundary. Select the intended validated outer-boundary candidate.',
  )
  assert.equal(
    foldImportWarningMessage(
      'FOLD仕様バージョンの記載がありません。対応範囲として慎重に解釈します。',
      'en',
    ),
    'The FOLD specification version is missing, so the file will be interpreted conservatively within the supported range.',
  )
  assert.equal(
    foldImportWarningMessage(
      'F（平らな折り筋）は同じ意味の線種がないため、補助線または除外へ変換します。',
      'en',
    ),
    'F (flat crease) has no equivalent line type and must be converted to an auxiliary line or excluded.',
  )
  assert.equal(
    foldImportWarningMessage(
      '取り込まないFOLD情報: ファイル分類、その他の拡張フィールド2件。',
      'en',
    ),
    'Some FOLD metadata will not be imported.',
  )

  const privateWarning = String.raw`C:\Users\alice\private.fold`
  const fallback = foldImportWarningMessage(privateWarning, 'en')
  assert.equal(fallback, 'Some FOLD information will not be imported.')
  assert.doesNotMatch(fallback, /alice|private|[ぁ-んァ-ン一-龯]/u)
  const japaneseFallback = foldImportWarningMessage(privateWarning, 'ja')
  assert.equal(japaneseFallback, '取り込まれないFOLD情報があります。')
  assert.doesNotMatch(japaneseFallback, /alice|private/u)
  const disguisedPrivateWarning =
    String.raw`取り込まないFOLD情報: C:\Users\alice\private.fold。`
  assert.equal(
    foldImportWarningMessage(disguisedPrivateWarning, 'ja'),
    '取り込まれないFOLD情報があります。',
  )
  assert.equal(
    foldImportWarningMessage(disguisedPrivateWarning, 'en'),
    'Some FOLD information will not be imported.',
  )

  assert.equal(foldImportSuggestedName('FOLDインポート', 'en'), 'FOLD import')
  assert.equal(foldImportSuggestedName('FOLDインポート', 'ja'), 'FOLDインポート')
  assert.equal(foldImportSuggestedName('鶴', 'en'), '鶴')
  assert.equal(
    foldImportPreviewFileName('選択したFOLDファイル', 'en'),
    'Selected FOLD file',
  )
  assert.equal(foldImportPreviewFileName('crane.fold', 'en'), 'crane.fold')
  assert.equal(
    foldImportPreviewFileName(String.raw`C:\Users\alice\private.fold`, 'en'),
    'Selected FOLD file',
  )
  for (const deceptive of [
    'private\u0085.fold',
    'private\u202e.dlof',
    'private\u2028.fold',
    'private\u2029.fold',
  ]) {
    assert.equal(
      foldImportPreviewFileName(deceptive, 'en'),
      'Selected FOLD file',
    )
  }
})

test('FOLD boundary candidates require a real preview-local ID and expose only selected edges', () => {
  const preview = {
    boundary_candidates: [
      {
        id: 0,
        source: 'assigned_boundary',
        edge_indices: [0, 1, 2, 3],
      },
      {
        id: 7,
        source: 'inferred_outer_face',
        edge_indices: [4, 5, 6],
      },
    ],
    fixed_boundary_candidate_id: 0,
  } as Pick<
    FoldImportPreview,
    'boundary_candidates' | 'fixed_boundary_candidate_id'
  >
  assert.equal(initialFoldBoundaryCandidateId(preview), 0)
  assert.equal(foldBoundaryCandidate(preview, 7)?.source, 'inferred_outer_face')
  assert.equal(foldBoundaryCandidate(preview, 65_536), null)
  assert.deepEqual([...foldBoundaryPreviewEdgeSet(preview, 7)], [4, 5, 6])
  assert.equal(
    foldBoundaryCandidateLabel(preview.boundary_candidates[0], 'ja'),
    '元のB線による外周（4辺）',
  )
  assert.equal(
    foldBoundaryCandidateLabel(preview.boundary_candidates[1], 'en'),
    'Validated boundary candidate 8 (3 edges)',
  )

  assert.equal(
    initialFoldBoundaryCandidateId({
      ...preview,
      fixed_boundary_candidate_id: 8,
    }),
    null,
  )
  assert.equal(
    initialFoldBoundaryCandidateId({
      ...preview,
      fixed_boundary_candidate_id: null,
    }),
    null,
  )
})

test('invalid or missing mappings remain unresolved', () => {
  assert.deepEqual(
    unresolvedFoldAssignments(
      [{ assignment: 'F', count: 1 }],
      { F: 'mountain' },
    ),
    ['F'],
  )
  assert.deepEqual(
    unresolvedFoldAssignments(
      [{ assignment: 'F', count: 0 }],
      {},
    ),
    [],
  )
})

test('FOLD scale parser accepts only a bounded positive finite conversion', () => {
  assert.equal(parseFoldImportScale('25.4'), 25.4)
  assert.equal(parseFoldImportScale('0.000001'), 0.000001)
  for (const invalid of ['', '0', '-1', 'Infinity', 'NaN', '1000000001']) {
    assert.equal(parseFoldImportScale(invalid), null)
  }
})

test('FOLD imported project names enforce the native name boundary', () => {
  assert.equal(isValidFoldImportName('  鶴  '), true)
  assert.equal(isValidFoldImportName(''), false)
  assert.equal(isValidFoldImportName(' \n '), false)
  assert.equal(isValidFoldImportName('a'.repeat(120)), true)
  assert.equal(isValidFoldImportName('a'.repeat(121)), false)
})

test('FOLD preview bounds include degenerate spans without producing a zero viewBox', () => {
  assert.equal(foldPreviewBounds([]), null)
  assert.deepEqual(foldPreviewBounds([{ x: 2, y: 3 }, { x: 12, y: 8 }]), {
    minX: 2,
    minY: 3,
    width: 10,
    height: 5,
  })
  assert.deepEqual(foldPreviewBounds([{ x: 2, y: 3 }]), {
    minX: 1.995,
    minY: 2.995,
    width: 0.01,
    height: 0.01,
  })
  assert.equal(foldPreviewBounds([{ x: Number.NaN, y: 0 }]), null)
  assert.equal(
    foldPreviewBounds([
      { x: -Number.MAX_VALUE, y: 0 },
      { x: Number.MAX_VALUE, y: 1 },
    ]),
    null,
  )
})
