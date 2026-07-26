import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  INSTRUCTION_TIMELINE_PRESENTATION_TEXT as TEXT,
} from '../src/lib/instructionTimelinePresentationText.ts'

const ROOT_KEYS = [
  'playback',
  'stopped',
  'notices',
  'capture',
  'editor',
  'duration',
] as const

const GROUP_KEYS = {
  playback: [
    'idle',
    'applying',
    'holding',
    'complete',
  ],
  stopped: [
    'stale_step',
    'project_changed',
    'revision_changed',
    'model_changed',
    'manual_pose',
    'benchmark',
    'file_operation',
    'apply_failed',
    'hidden',
    'disposed',
    'canceled',
  ],
  notices: [
    'add_failed',
    'added',
    'updated',
    'update_failed',
    'pose_updated',
    'pose_update_failed',
    'delete_failed',
    'deleted',
    'moved',
    'split',
    'merged',
    'move_failed',
    'stale_pose',
    'pose_apply_failed',
    'pose_applying',
    'model_required',
    'no_steps',
    'declarative_playback_unsupported',
  ],
  capture: [
    'project_required',
    'pose_required',
    'pose_running',
    'pose_invalid',
    'pose_blocked',
    'pose_indeterminate',
    'pose_ready',
  ],
  editor: [
    'invalid_metadata',
    'update_failed',
  ],
  duration: [
    'seconds',
    'numberLocale',
  ],
} as const

const EXPECTED_TEXT = {
  playback: {
    idle: {
      ja: '再生停止中',
      en: 'Playback stopped',
    },
    applying: {
      ja: '手順 {step}「{title}」を表示しています',
      en: 'Applying step {step}, “{title}”',
    },
    holding: {
      ja: '手順 {step}「{title}」を表示中です',
      en: 'Showing step {step}, “{title}”',
    },
    complete: {
      ja: '折り手順の段階再生が完了しました',
      en: 'Finished playing all folding steps',
    },
  },
  stopped: {
    stale_step: {
      ja: '展開図が変わった手順のため再生を停止しました',
      en: 'Playback stopped because the crease pattern changed for this step',
    },
    project_changed: {
      ja: 'プロジェクトが変わったため再生を停止しました',
      en: 'Playback stopped because the project changed',
    },
    revision_changed: {
      ja: '編集中の内容が変わったため再生を停止しました',
      en: 'Playback stopped because the edited content changed',
    },
    model_changed: {
      ja: '3Dモデルが変わったため再生を停止しました',
      en: 'Playback stopped because the 3D model changed',
    },
    manual_pose: {
      ja: '3D姿勢を手動変更したため再生を停止しました',
      en: 'Playback stopped because the 3D pose was changed manually',
    },
    benchmark: {
      ja: '性能テストを開始したため再生を停止しました',
      en: 'Playback stopped because a performance test started',
    },
    file_operation: {
      ja: 'ファイル操作を開始したため再生を停止しました',
      en: 'Playback stopped because a file operation started',
    },
    apply_failed: {
      ja: '3D姿勢を適用できなかったため再生を停止しました',
      en: 'Playback stopped because the 3D pose could not be applied',
    },
    hidden: {
      ja: '画面が非表示になったため再生を停止しました',
      en: 'Playback stopped because the window became hidden',
    },
    disposed: {
      ja: '画面を閉じたため再生を停止しました',
      en: 'Playback stopped because the view was closed',
    },
    canceled: {
      ja: '折り手順の再生を停止しました',
      en: 'Folding-step playback stopped',
    },
  },
  notices: {
    add_failed: {
      ja: '現在の3D姿勢を手順へ追加できませんでした',
      en: 'Could not add the current 3D pose as a step',
    },
    added: {
      ja: '「{title}」を追加しました',
      en: 'Added “{title}”',
    },
    updated: {
      ja: '「{title}」を更新しました',
      en: 'Updated “{title}”',
    },
    update_failed: {
      ja: '手順を更新できませんでした',
      en: 'Could not update the step',
    },
    pose_updated: {
      ja: '「{title}」の姿勢を現在の3D表示で更新しました',
      en: 'Updated the pose for “{title}” from the current 3D view',
    },
    pose_update_failed: {
      ja: '手順の姿勢を更新できませんでした',
      en: 'Could not update the step pose',
    },
    delete_failed: {
      ja: '手順を削除できませんでした',
      en: 'Could not delete the step',
    },
    deleted: {
      ja: '「{title}」を削除しました',
      en: 'Deleted “{title}”',
    },
    moved: {
      ja: '手順の順番を変更しました',
      en: 'Changed the step order',
    },
    split: {
      ja: '手順を分割しました',
      en: 'Split the step',
    },
    merged: {
      ja: '手順を次の手順と結合しました',
      en: 'Merged the step with the next step',
    },
    move_failed: {
      ja: '手順を移動できませんでした',
      en: 'Could not move the step',
    },
    stale_pose: {
      ja: '展開図が変更された手順です。「現在の3D姿勢で更新」してから表示してください',
      en: 'The crease pattern changed for this step. Update it with the current 3D pose before showing it.',
    },
    pose_apply_failed: {
      ja: 'この手順の姿勢は現在の3Dモデルへ適用できません',
      en: 'This step pose cannot be applied to the current 3D model',
    },
    pose_applying: {
      ja: '「{title}」の保存姿勢を3Dへ適用しています',
      en: 'Applying the saved pose for “{title}” to the 3D view',
    },
    model_required: {
      ja: '再生できる3Dモデルを準備してください',
      en: 'Prepare a 3D model that can be played',
    },
    no_steps: {
      ja: '再生する手順がありません',
      en: 'There are no steps to play',
    },
    declarative_playback_unsupported: {
      ja: '説明専用ステップは3D姿勢を持たないため再生できません。内容は一覧で確認してください',
      en: 'Description-only steps have no 3D pose and cannot be played. Review them in the timeline list.',
    },
  },
  capture: {
    project_required: {
      ja: 'プロジェクトを読み込んでください。',
      en: 'Open a project first.',
    },
    pose_required: {
      ja: '現在のrevisionの3D表示を準備しています。',
      en: 'Preparing the 3D view for the current revision.',
    },
    pose_running: {
      ja: '3Dの動作が止まってから記録できます。',
      en: 'Wait for the 3D motion to stop before recording.',
    },
    pose_invalid: {
      ja: '現在の3D姿勢は手順として安全に読み取れません。',
      en: 'The current 3D pose cannot be read safely as a step.',
    },
    pose_blocked: {
      ja: '衝突境界で安全に停止している表示姿勢を記録します。',
      en: 'Records the displayed pose that stopped safely at a collision boundary.',
    },
    pose_indeterminate: {
      ja: '経路判定不能で停止した現在の表示姿勢だけを記録します。',
      en: 'Records only the current displayed pose that stopped because the path was indeterminate.',
    },
    pose_ready: {
      ja: '現在3Dに安全に表示されている姿勢を記録します。',
      en: 'Records the pose currently shown safely in 3D.',
    },
  },
  editor: {
    invalid_metadata: {
      ja: 'タイトルは必須・改行なし{titleMaximum}文字以内、表示時間は{durationMinimum}〜{durationMaximum}msです。',
      en: 'The title is required, must be one line, and must be at most {titleMaximum} characters. Display time must be {durationMinimum}–{durationMaximum} ms.',
    },
    update_failed: {
      ja: '手順の説明を更新できませんでした',
      en: 'Could not update the step details',
    },
  },
  duration: {
    seconds: {
      ja: '{seconds}秒',
      en: '{seconds} seconds',
    },
    numberLocale: {
      ja: 'ja-JP',
      en: 'en-US',
    },
  },
} as const

const EXPECTED_PLACEHOLDERS = {
  'playback.applying': ['step', 'title'],
  'playback.holding': ['step', 'title'],
  'notices.added': ['title'],
  'notices.updated': ['title'],
  'notices.pose_updated': ['title'],
  'notices.deleted': ['title'],
  'notices.pose_applying': ['title'],
  'editor.invalid_metadata': [
    'titleMaximum',
    'durationMinimum',
    'durationMaximum',
  ],
  'duration.seconds': ['seconds'],
} as const

type LocalizedLeaf = Readonly<{
  ja: string
  en: string
}>

type CatalogRecord = Readonly<
  Record<string, Readonly<Record<string, LocalizedLeaf>>>
>

test('instruction timeline catalog has exact ordered keys, values, and 44 leaves', () => {
  assert.deepEqual(Reflect.ownKeys(TEXT), ROOT_KEYS)

  let leafCount = 0
  for (const groupKey of ROOT_KEYS) {
    const group = TEXT[groupKey]
    assert.deepEqual(
      Reflect.ownKeys(group),
      GROUP_KEYS[groupKey],
      groupKey,
    )
    leafCount += Reflect.ownKeys(group).length
  }

  assert.equal(leafCount, 44)
  assert.deepEqual(TEXT, EXPECTED_TEXT)
})

test('every catalog node is frozen and every locale is an own data property', () => {
  assert.equal(Object.isFrozen(TEXT), true, 'root')

  for (const groupKey of ROOT_KEYS) {
    const group = TEXT[groupKey]
    assert.equal(Object.isFrozen(group), true, groupKey)
    assertFrozenDataProperty(TEXT, groupKey, group)

    for (const leafKey of GROUP_KEYS[groupKey]) {
      const leaf = group[leafKey]
      const path = `${groupKey}.${leafKey}`
      assert.equal(Object.isFrozen(leaf), true, path)
      assert.equal(isExactLocalizedLeaf(leaf), true, path)
      assert.deepEqual(Reflect.ownKeys(leaf), ['ja', 'en'], path)

      const descriptors = Object.getOwnPropertyDescriptors(leaf)
      assert.deepEqual(Reflect.ownKeys(descriptors), ['ja', 'en'], path)
      for (const locale of ['ja', 'en'] as const) {
        const descriptor = descriptors[locale]
        assert.ok(descriptor, `${path}.${locale}`)
        assert.equal('value' in descriptor, true, `${path}.${locale}`)
        assert.equal('get' in descriptor, false, `${path}.${locale}`)
        assert.equal('set' in descriptor, false, `${path}.${locale}`)
        assert.equal(typeof descriptor.value, 'string', `${path}.${locale}`)
        assert.equal(descriptor.enumerable, true, `${path}.${locale}`)
        assert.equal(descriptor.configurable, false, `${path}.${locale}`)
        assert.equal(descriptor.writable, false, `${path}.${locale}`)
      }

      assertFrozenDataProperty(group, leafKey, leaf)
    }
  }
})

test('locale-leaf inspection rejects inherited, extra, symbol, accessor, and Proxy inputs', () => {
  assert.equal(
    isExactLocalizedLeaf(Object.create({
      ja: 'inherited Japanese',
      en: 'inherited English',
    })),
    false,
  )
  assert.equal(
    isExactLocalizedLeaf({
      ja: 'Japanese',
      en: 'English',
      extra: 'forged',
    }),
    false,
  )

  const symbol = Symbol('forged-locale')
  assert.equal(
    isExactLocalizedLeaf({
      ja: 'Japanese',
      en: 'English',
      [symbol]: 'forged',
    }),
    false,
  )

  let accessorWasRead = false
  const accessorLeaf = Object.defineProperties({}, {
    ja: {
      enumerable: true,
      get: () => {
        accessorWasRead = true
        throw new Error('the ja accessor must not run')
      },
    },
    en: {
      enumerable: true,
      get: () => {
        accessorWasRead = true
        throw new Error('the en accessor must not run')
      },
    },
  })
  assert.equal(isExactLocalizedLeaf(accessorLeaf), false)
  assert.equal(accessorWasRead, false)

  const trapError = new Error('raw hostile Proxy trap')
  const throwingProxy = new Proxy(Object.create(null), {
    getPrototypeOf: () => {
      throw trapError
    },
    setPrototypeOf: () => {
      throw trapError
    },
    isExtensible: () => {
      throw trapError
    },
    preventExtensions: () => {
      throw trapError
    },
    getOwnPropertyDescriptor: () => {
      throw trapError
    },
    defineProperty: () => {
      throw trapError
    },
    has: () => {
      throw trapError
    },
    get: () => {
      throw trapError
    },
    set: () => {
      throw trapError
    },
    deleteProperty: () => {
      throw trapError
    },
    ownKeys: () => {
      throw trapError
    },
  })
  assert.doesNotThrow(() => {
    assert.equal(isExactLocalizedLeaf(throwingProxy), false)
  })
})

test('exactly 9 leaves contain locale-equivalent placeholder multisets and 35 contain none', () => {
  const actualPlaceholderPaths: string[] = []
  let emptyLeafCount = 0

  for (const [path, leaf] of catalogLeaves(TEXT)) {
    const ja = placeholders(leaf.ja)
    const en = placeholders(leaf.en)
    assert.deepEqual(
      [...ja].sort(),
      [...en].sort(),
      `${path} locale multiset`,
    )

    const expected = EXPECTED_PLACEHOLDERS[
      path as keyof typeof EXPECTED_PLACEHOLDERS
    ]
    if (expected) {
      actualPlaceholderPaths.push(path)
      assert.deepEqual(ja, expected, `${path}.ja`)
      assert.deepEqual(en, expected, `${path}.en`)
    } else {
      emptyLeafCount += 1
      assert.deepEqual(ja, [], `${path}.ja`)
      assert.deepEqual(en, [], `${path}.en`)
    }
  }

  assert.deepEqual(
    actualPlaceholderPaths,
    Reflect.ownKeys(EXPECTED_PLACEHOLDERS),
  )
  assert.equal(actualPlaceholderPaths.length, 9)
  assert.equal(emptyLeafCount, 35)
})

test('source-order pair and canonical catalog hashes are exact', () => {
  const catalog = TEXT as CatalogRecord
  const sourceOrderPairs = [
    ...Object.values(catalog.playback),
    ...Object.values(catalog.notices),
    ...Object.values(catalog.editor),
    ...Object.values(catalog.capture),
    ...Object.values(catalog.duration),
    ...Object.values(catalog.stopped),
  ]
  assert.equal(sourceOrderPairs.length, 44)
  assert.equal(
    sha256(JSON.stringify(sourceOrderPairs)),
    '735394adb353138c9dfbb3179852bd69184f799a271f41607ee5f404dda7867a',
  )
  assert.equal(
    sha256(JSON.stringify(TEXT)),
    'b2089960622903710b5f562fc5205dc5f601f96fe342506f2a88a70b6ff4cb88',
  )
})

test('catalog source has no Tauri, DOM, mutation callback, locale branch, or helper export', () => {
  const source = readFileSync(
    new URL(
      '../src/lib/instructionTimelinePresentationText.ts',
      import.meta.url,
    ),
    'utf8',
  )

  assert.doesNotMatch(source, /@tauri-apps\/api/iu)
  assert.doesNotMatch(source, /\binvoke\s*\(/u)
  assert.doesNotMatch(source, /\b(?:document|window)\s*(?:\.|\[)/u)
  assert.doesNotMatch(
    source,
    /\b(?:callback|mutate|dispatch|setState|on[A-Z][A-Za-z0-9_]*)\b/u,
  )
  assert.doesNotMatch(
    source,
    /\b(?:if|switch)\s*\([^)]*\blocale\b/iu,
  )
  assert.doesNotMatch(
    source,
    /\blocale\s*(?:===|!==|==|!=|\?)/iu,
  )

  const exportedDeclarations = [
    ...source.matchAll(
      /\bexport\s+(?:declare\s+)?(?:const|let|var|function|class|type|interface|enum)\s+([A-Za-z_$][A-Za-z0-9_$]*)/gu,
    ),
  ].map((match) => match[1])
  assert.deepEqual(exportedDeclarations, [
    'INSTRUCTION_TIMELINE_PRESENTATION_TEXT',
  ])
  assert.doesNotMatch(source, /\bexport\s+default\b/u)
  assert.doesNotMatch(source, /\bexport\s+\*/u)
  assert.doesNotMatch(source, /\bexport\s*\{/u)
})

function assertFrozenDataProperty(
  owner: object,
  key: PropertyKey,
  expectedValue: unknown,
) {
  const descriptor = Object.getOwnPropertyDescriptor(owner, key)
  assert.ok(descriptor, String(key))
  assert.equal('value' in descriptor, true, String(key))
  assert.equal(descriptor.value, expectedValue, String(key))
  assert.equal(descriptor.enumerable, true, String(key))
  assert.equal(descriptor.configurable, false, String(key))
  assert.equal(descriptor.writable, false, String(key))
}

function isExactLocalizedLeaf(value: unknown): value is LocalizedLeaf {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    return false
  }
  try {
    if (!Object.isFrozen(value)) return false
    const keys = Reflect.ownKeys(value)
    if (keys.length !== 2 || keys[0] !== 'ja' || keys[1] !== 'en') {
      return false
    }
    const descriptors = Object.getOwnPropertyDescriptors(value)
    return (['ja', 'en'] as const).every((locale) => {
      const descriptor = descriptors[locale]
      return descriptor !== undefined
        && 'value' in descriptor
        && !('get' in descriptor)
        && !('set' in descriptor)
        && typeof descriptor.value === 'string'
        && descriptor.enumerable === true
        && descriptor.configurable === false
        && descriptor.writable === false
    })
  } catch {
    return false
  }
}

function catalogLeaves(
  catalog: typeof TEXT,
): readonly (readonly [string, LocalizedLeaf])[] {
  const result: [string, LocalizedLeaf][] = []
  for (const groupKey of ROOT_KEYS) {
    const group = catalog[groupKey]
    for (const leafKey of GROUP_KEYS[groupKey]) {
      result.push([`${groupKey}.${leafKey}`, group[leafKey]])
    }
  }
  return result
}

function placeholders(value: string): string[] {
  return [...value.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\}/gu)]
    .map((match) => match[1]!)
}

function sha256(value: string): string {
  return createHash('sha256').update(value, 'utf8').digest('hex')
}
