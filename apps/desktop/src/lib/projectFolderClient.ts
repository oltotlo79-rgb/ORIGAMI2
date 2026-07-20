import { invoke } from '@tauri-apps/api/core'

import type { ProjectFileResponse } from './coreClient.ts'
import type { Locale } from './i18n.ts'
import { parsePathlessProjectSnapshot } from './recoveryClient.ts'

export type ProjectFolderClientErrorCode =
  | 'native_unavailable'
  | 'busy'
  | 'invalid_request'
  | 'open_failed'
  | 'invalid'
  | 'too_large'
  | 'link_or_special_entry'
  | 'changed_during_read'
  | 'save_failed'
  | 'target_exists'
  | 'project_changed'
  | 'recovery_required'
  | 'replacement_unsupported'
  | 'invalid_response'

export type ProjectFolderNativeInvoke = (
  command: string,
  args: Readonly<Record<string, unknown>>,
) => Promise<unknown>

export type ProjectFolderClient = Readonly<{
  open: (locale: Locale) => Promise<ProjectFileResponse>
  saveAsNew: (locale: Locale) => Promise<ProjectFileResponse>
}>

const NATIVE_ERROR_CODES = Object.freeze({
  project_folder_busy: 'busy',
  project_folder_invalid_request: 'invalid_request',
  project_folder_open_failed: 'open_failed',
  project_folder_invalid: 'invalid',
  project_folder_too_large: 'too_large',
  project_folder_link_or_special_entry: 'link_or_special_entry',
  project_folder_changed_during_read: 'changed_during_read',
  project_folder_save_failed: 'save_failed',
  project_folder_target_exists: 'target_exists',
  project_folder_project_changed: 'project_changed',
  project_folder_recovery_required: 'recovery_required',
  project_folder_replacement_unsupported: 'replacement_unsupported',
} satisfies Record<string, ProjectFolderClientErrorCode>)

export class ProjectFolderClientError extends Error {
  readonly code: ProjectFolderClientErrorCode

  constructor(code: ProjectFolderClientErrorCode) {
    super(code)
    this.name = 'ProjectFolderClientError'
    this.code = code
  }
}

const defaultNativeInvoke: ProjectFolderNativeInvoke = (command, args) =>
  invoke<unknown>(command, { ...args })

const defaultClient = createProjectFolderClient()

export function createProjectFolderClient(
  nativeInvoke: ProjectFolderNativeInvoke = defaultNativeInvoke,
  nativeAvailable: () => boolean = isNativeProjectFolderAvailable,
): ProjectFolderClient {
  const run = async (
    command: 'open_project_folder' | 'save_project_folder_as',
    locale: Locale,
  ): Promise<ProjectFileResponse> => {
    if (locale !== 'ja' && locale !== 'en') {
      throw new ProjectFolderClientError('invalid_response')
    }
    if (!nativeAvailable()) {
      throw new ProjectFolderClientError('native_unavailable')
    }
    try {
      const response = await nativeInvoke(command, Object.freeze({ locale }))
      return normalizeProjectFolderResponse(response)
    } catch (error) {
      throw mapNativeError(error)
    }
  }
  return Object.freeze({
    open: (locale) => run('open_project_folder', locale),
    saveAsNew: (locale) => run('save_project_folder_as', locale),
  })
}

export function isNativeProjectFolderAvailable(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

export function openProjectFolder(locale: Locale): Promise<ProjectFileResponse> {
  return defaultClient.open(locale)
}

export function saveProjectFolderAs(
  locale: Locale,
): Promise<ProjectFileResponse> {
  return defaultClient.saveAsNew(locale)
}

export function normalizeProjectFolderResponse(
  value: unknown,
): ProjectFileResponse {
  const record = exactRecord(value, ['canceled', 'project'])
  if (typeof record.canceled !== 'boolean') {
    throw new ProjectFolderClientError('invalid_response')
  }
  const project = parsePathlessProjectSnapshot(record.project)
  if (!project) throw new ProjectFolderClientError('invalid_response')
  return Object.freeze({
    canceled: record.canceled,
    project,
  })
}

export function projectFolderClientErrorCode(
  error: unknown,
): ProjectFolderClientErrorCode {
  return error instanceof ProjectFolderClientError
    ? error.code
    : 'invalid_response'
}

export function projectFolderClientErrorMessage(
  error: unknown,
  locale: Locale,
): string {
  const code = projectFolderClientErrorCode(error)
  const messages: Readonly<Record<
    ProjectFolderClientErrorCode,
    Readonly<Record<Locale, string>>
  >> = {
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
  }
  return messages[code][locale]
}

function exactRecord(
  value: unknown,
  keys: readonly string[],
): Record<string, unknown> {
  try {
    if (typeof value !== 'object' || value === null || Array.isArray(value)) {
      throw new ProjectFolderClientError('invalid_response')
    }
    const prototype = Object.getPrototypeOf(value)
    if (prototype !== Object.prototype && prototype !== null) {
      throw new ProjectFolderClientError('invalid_response')
    }
    const descriptors = Object.getOwnPropertyDescriptors(value)
    const ownKeys = Reflect.ownKeys(descriptors)
    if (
      ownKeys.length !== keys.length
      || ownKeys.some((key) =>
        typeof key !== 'string' || !keys.includes(key))
    ) throw new ProjectFolderClientError('invalid_response')
    const snapshot: Record<string, unknown> = Object.create(null)
    for (const key of keys) {
      const descriptor = descriptors[key]
      if (!descriptor || !descriptor.enumerable || !('value' in descriptor)) {
        throw new ProjectFolderClientError('invalid_response')
      }
      snapshot[key] = descriptor.value
    }
    return snapshot
  } catch (error) {
    if (error instanceof ProjectFolderClientError) throw error
    throw new ProjectFolderClientError('invalid_response')
  }
}

function mapNativeError(error: unknown): ProjectFolderClientError {
  if (error instanceof ProjectFolderClientError) return error
  if (typeof error === 'string' && Object.hasOwn(NATIVE_ERROR_CODES, error)) {
    return new ProjectFolderClientError(
      NATIVE_ERROR_CODES[error as keyof typeof NATIVE_ERROR_CODES],
    )
  }
  return new ProjectFolderClientError('invalid_response')
}
