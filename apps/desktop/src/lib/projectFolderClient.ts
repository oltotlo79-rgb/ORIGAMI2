import { invoke } from '@tauri-apps/api/core'

import type { ProjectFileResponse } from './coreClient.ts'
import type { Locale } from './i18n.ts'
import { PROJECT_FOLDER_CLIENT_TEXT } from './projectFolderClientText.ts'
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
  try {
    if (!(error instanceof ProjectFolderClientError)) {
      return 'invalid_response'
    }
    const code: unknown = error.code
    return typeof code === 'string'
      && Object.hasOwn(PROJECT_FOLDER_CLIENT_TEXT, code)
      ? code as ProjectFolderClientErrorCode
      : 'invalid_response'
  } catch {
    return 'invalid_response'
  }
}

export function projectFolderClientErrorMessage(
  error: unknown,
  locale: Locale,
): string {
  const code = projectFolderClientErrorCode(error)
  return PROJECT_FOLDER_CLIENT_TEXT[code][locale]
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
  try {
    if (error instanceof ProjectFolderClientError) return error
  } catch {
    return new ProjectFolderClientError('invalid_response')
  }
  if (typeof error === 'string' && Object.hasOwn(NATIVE_ERROR_CODES, error)) {
    return new ProjectFolderClientError(
      NATIVE_ERROR_CODES[error as keyof typeof NATIVE_ERROR_CODES],
    )
  }
  return new ProjectFolderClientError('invalid_response')
}
