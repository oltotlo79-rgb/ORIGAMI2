import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

import {
  ProjectFolderClientError,
  projectFolderClientErrorCode,
  projectFolderClientErrorMessage,
  type ProjectFolderClientErrorCode,
} from '../src/lib/projectFolderClient.ts'
import {
  PROJECT_FOLDER_CLIENT_TEXT as TEXT,
  type ProjectFolderClientText,
} from '../src/lib/projectFolderClientText.ts'
import type { Locale } from '../src/lib/i18n.ts'

const ERROR_CODES = [
  'native_unavailable',
  'busy',
  'invalid_request',
  'open_failed',
  'invalid',
  'too_large',
  'link_or_special_entry',
  'changed_during_read',
  'save_failed',
  'target_exists',
  'project_changed',
  'recovery_required',
  'replacement_unsupported',
  'invalid_response',
] as const satisfies readonly ProjectFolderClientErrorCode[]

const EXPECTED_TEXT = {
  native_unavailable: {
    ja: '展開フォルダー操作はデスクトップ版で利用できます。',
    en: 'Expanded-folder operations are available in the desktop app.',
  },
  busy: {
    ja: '別の展開フォルダー操作を処理中です。完了後にもう一度実行してください。',
    en: 'Another expanded-folder operation is running. Try again after it finishes.',
  },
  invalid_request: {
    ja: '展開フォルダー操作の条件を確認できませんでした。もう一度実行してください。',
    en: 'The expanded-folder request could not be verified. Try again.',
  },
  open_failed: {
    ja: '選択した展開フォルダーを安全に開けませんでした。アクセス権を確認してください。',
    en: 'The selected expanded folder could not be opened safely. Check its permissions.',
  },
  invalid: {
    ja: '展開フォルダーのmanifestまたはプロジェクト内容が正しくありません。',
    en: 'The expanded folder has an invalid manifest or project content.',
  },
  too_large: {
    ja: '展開フォルダー内のファイルがサイズ上限を超えています。',
    en: 'A file in the expanded folder exceeds the size limit.',
  },
  link_or_special_entry: {
    ja: '展開フォルダーにリンク、再解析ポイント、ハードリンク、または特殊ファイルが含まれています。通常のファイルだけにしてください。',
    en: 'The expanded folder contains a link, reparse point, hard link, or special file. Use ordinary files only.',
  },
  changed_during_read: {
    ja: '処理中に展開フォルダーが変更されました。変更が止まってからもう一度実行してください。',
    en: 'The expanded folder changed during processing. Try again after changes stop.',
  },
  save_failed: {
    ja: '展開フォルダーを安全に保存できませんでした。保存先のアクセス権と空き容量を確認してください。',
    en: 'The expanded folder could not be saved safely. Check destination permissions and free space.',
  },
  target_exists: {
    ja: '同じ名前の展開フォルダーは別のプロジェクトに属するか、安全な置き換え条件を満たしていません。別の親フォルダーを選んでください。',
    en: 'The same-named expanded folder belongs to another project or cannot be replaced safely. Choose a different parent folder.',
  },
  project_changed: {
    ja: '操作中にプロジェクトが変更されました。現在の内容でもう一度実行してください。',
    en: 'The project changed during the operation. Try again with the current content.',
  },
  recovery_required: {
    ja: '前回の展開フォルダー置き換えを安全に完了する必要があります。保存先が外付けドライブ等にある場合は再接続してから、展開フォルダー操作をもう一度実行してください。',
    en: 'A previous expanded-folder replacement must be recovered safely. If its destination is on an external drive, reconnect it and retry an expanded-folder operation.',
  },
  replacement_unsupported: {
    ja: 'この保存先では既存フォルダーの安全な置き換えを保証できません。新しいフォルダー名で保存するか、ローカルのNTFS/ReFS保存先を選んでください。',
    en: 'Safe replacement of an existing folder cannot be guaranteed at this destination. Save with a new folder name or choose a local NTFS/ReFS destination.',
  },
  invalid_response: {
    ja: '展開フォルダー操作の応答を確認できませんでした。もう一度実行してください。',
    en: 'The expanded-folder response could not be verified. Try again.',
  },
} satisfies ProjectFolderClientText

test('project-folder client catalog is exact, stable, and deeply frozen', () => {
  assert.deepEqual(Object.keys(TEXT), ERROR_CODES)
  assert.deepEqual(TEXT, EXPECTED_TEXT)
  assertDeepFrozen(TEXT)

  for (const code of ERROR_CODES) {
    const localized = TEXT[code]
    assert.deepEqual(Object.keys(localized), ['ja', 'en'])
    assert.deepEqual(placeholders(localized.ja), [])
    assert.deepEqual(placeholders(localized.en), [])
  }

  assert.equal(
    createHash('sha256')
      .update(JSON.stringify(TEXT), 'utf8')
      .digest('hex'),
    '6372f8cbfa91bf0377183ef03bac47e1916b3f49acca011bca680994a76b01af',
  )
})

test('all fourteen codes preserve their exact Japanese and English output', () => {
  for (const code of ERROR_CODES) {
    const error = new ProjectFolderClientError(code)
    assert.equal(projectFolderClientErrorCode(error), code)
    assert.equal(
      projectFolderClientErrorMessage(error, 'ja'),
      EXPECTED_TEXT[code].ja,
    )
    assert.equal(
      projectFolderClientErrorMessage(error, 'en'),
      EXPECTED_TEXT[code].en,
    )
  }
})

test('unknown runtime locales preserve the existing undefined boundary', () => {
  const error = new ProjectFolderClientError('busy')
  for (const locale of [
    'fr',
    '',
    null,
    undefined,
    42,
    Symbol('locale'),
  ]) {
    assert.equal(
      projectFolderClientErrorMessage(error, locale as Locale),
      undefined,
    )
  }
})

test('hostile errors and secret paths collapse to the fixed redacted response', () => {
  const privatePath = String.raw`C:\Users\alice\秘密\project\manifest.json`
  const hostileError = new Proxy(Object.create(null) as object, {
    getPrototypeOf() {
      throw new Error(privatePath)
    },
    get() {
      throw new Error(privatePath)
    },
  })
  const tampered = new ProjectFolderClientError('busy')
  Object.defineProperty(tampered, 'code', {
    enumerable: true,
    get() {
      throw new Error(privatePath)
    },
  })

  for (const error of [
    privatePath,
    new Error(privatePath),
    { code: privatePath, path: privatePath },
    hostileError,
    tampered,
  ]) {
    assert.equal(projectFolderClientErrorCode(error), 'invalid_response')
    for (const locale of ['ja', 'en'] as const) {
      const output = projectFolderClientErrorMessage(error, locale)
      assert.equal(output, EXPECTED_TEXT.invalid_response[locale])
      assert.doesNotMatch(output, /alice|manifest|project|秘密|Users/iu)
    }
  }
})

test('the client delegates presentation literals to the dedicated catalog', async () => {
  const consumer = await readFile(
    new URL('../src/lib/projectFolderClient.ts', import.meta.url),
    'utf8',
  )
  const formatter = functionSection(
    consumer,
    'export function projectFolderClientErrorMessage(',
    'function exactRecord(',
  )

  assert.match(
    consumer,
    /import \{ PROJECT_FOLDER_CLIENT_TEXT \} from '\.\/projectFolderClientText\.ts'/u,
  )
  assert.match(
    formatter,
    /return PROJECT_FOLDER_CLIENT_TEXT\[code\]\[locale\]/u,
  )
  assert.doesNotMatch(formatter, /\bmessages\b/u)
  assert.doesNotMatch(formatter, /[ぁ-んァ-ン一-龯]/u)
  assert.doesNotMatch(
    formatter,
    /Expanded-folder|expanded folder|Try again|desktop app/u,
  )
})

function assertDeepFrozen(value: unknown, seen = new Set<object>()): void {
  if (!value || typeof value !== 'object' || seen.has(value)) return
  seen.add(value)
  assert.equal(Object.isFrozen(value), true)
  for (const nested of Object.values(value)) {
    assertDeepFrozen(nested, seen)
  }
}

function placeholders(value: string): string[] {
  return [...value.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\}/gu)]
    .map((match) => match[1]!)
    .sort()
}

function functionSection(text: string, start: string, end: string) {
  const startIndex = text.indexOf(start)
  const endIndex = text.indexOf(end, startIndex + start.length)
  assert.ok(startIndex >= 0 && endIndex > startIndex, `${start} section`)
  return text.slice(startIndex, endIndex)
}
