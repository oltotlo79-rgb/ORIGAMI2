import { invoke } from '@tauri-apps/api/core'

import {
  parseGlobalFlatFoldabilityJobDto,
  type GlobalFlatFoldabilityErrorCategory,
  type GlobalFlatFoldabilityJobDto,
} from './globalFlatFoldability.ts'

const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/u
const MAX_TIME_LIMIT_MS = 300_000

export type GlobalFlatFoldabilityNativeContext = Readonly<{
  projectId: string
  revision: number
  foldModelFingerprint: string
}>

export type GlobalFlatFoldabilityNativeBegin = Readonly<{
  jobId: string
  job: GlobalFlatFoldabilityJobDto
}>

export type GlobalFlatFoldabilityNativeTransport = Readonly<{
  begin(
    context: GlobalFlatFoldabilityNativeContext,
    timeLimitMs: number,
  ): Promise<GlobalFlatFoldabilityNativeBegin>
  poll(jobId: string): Promise<GlobalFlatFoldabilityJobDto>
  cancel(jobId: string): Promise<void>
}>

export type GlobalFlatFoldabilityNativeInvoke = (
  command: string,
  arguments_?: Readonly<Record<string, unknown>>,
) => unknown

export class GlobalFlatFoldabilityNativeError extends Error {
  readonly category: GlobalFlatFoldabilityErrorCategory

  constructor(category: GlobalFlatFoldabilityErrorCategory) {
    super(category)
    this.name = 'GlobalFlatFoldabilityNativeError'
    this.category = category
  }
}

export function createGlobalFlatFoldabilityNativeTransport(
  nativeInvoke: GlobalFlatFoldabilityNativeInvoke = defaultNativeInvoke,
): GlobalFlatFoldabilityNativeTransport {
  return Object.freeze({
    async begin(context, timeLimitMs) {
      if (
        !isUuid(context.projectId)
        || !isRevision(context.revision)
        || !isFoldModelFingerprint(context.foldModelFingerprint)
        || !Number.isSafeInteger(timeLimitMs)
        || timeLimitMs < 1_000
        || timeLimitMs > MAX_TIME_LIMIT_MS
      ) {
        throw new GlobalFlatFoldabilityNativeError('invalid_request')
      }

      let raw: unknown
      try {
        raw = await Promise.resolve(nativeInvoke(
          'begin_global_flat_foldability',
          {
            expectedProjectId: context.projectId,
            expectedRevision: context.revision,
            expectedFoldModelFingerprint: context.foldModelFingerprint,
            timeLimitMs,
          },
        ))
      } catch (error) {
        throw new GlobalFlatFoldabilityNativeError(
          closedErrorCategory(error) ?? 'worker_unavailable',
        )
      }
      const record = exactDataRecord(raw, ['job_id', 'job'])
      const job = record
        ? parseGlobalFlatFoldabilityJobDto(record.job)
        : null
      if (
        !record
        || !isUuid(record.job_id)
        || !job
        || (
          job.state !== 'queued'
          && !isImmediateSourceLimitResult(job)
        )
      ) {
        throw new GlobalFlatFoldabilityNativeError('worker_unavailable')
      }
      return Object.freeze({ jobId: record.job_id, job })
    },

    async poll(jobId) {
      if (!isUuid(jobId)) {
        throw new GlobalFlatFoldabilityNativeError('invalid_request')
      }

      try {
        const rawResult = await Promise.resolve(nativeInvoke(
          'get_global_flat_foldability_result',
          { jobId },
        ))
        const result = parseGlobalFlatFoldabilityJobDto(rawResult)
        if (
          !result
          || result.state === 'queued'
          || result.state === 'running'
        ) {
          throw new GlobalFlatFoldabilityNativeError('result_unavailable')
        }
        return result
      } catch (error) {
        if (error instanceof GlobalFlatFoldabilityNativeError) throw error
        const category = closedErrorCategory(error)
        if (category !== 'result_unavailable') {
          throw new GlobalFlatFoldabilityNativeError(
            category ?? 'result_unavailable',
          )
        }
      }

      try {
        const rawProgress = await Promise.resolve(nativeInvoke(
          'get_global_flat_foldability_progress',
          { jobId },
        ))
        const progress = parseGlobalFlatFoldabilityJobDto(rawProgress)
        if (!progress) {
          throw new GlobalFlatFoldabilityNativeError('result_unavailable')
        }
        return progress
      } catch (error) {
        if (error instanceof GlobalFlatFoldabilityNativeError) throw error
        throw new GlobalFlatFoldabilityNativeError(
          closedErrorCategory(error) ?? 'result_unavailable',
        )
      }
    },

    async cancel(jobId) {
      if (!isUuid(jobId)) {
        throw new GlobalFlatFoldabilityNativeError('invalid_request')
      }
      try {
        await Promise.resolve(nativeInvoke(
          'cancel_global_flat_foldability',
          { jobId },
        ))
      } catch (error) {
        throw new GlobalFlatFoldabilityNativeError(
          closedErrorCategory(error) ?? 'result_unavailable',
        )
      }
    },
  })
}

function defaultNativeInvoke(
  command: string,
  arguments_?: Readonly<Record<string, unknown>>,
) {
  return invoke(command, arguments_)
}

function isUuid(value: unknown): value is string {
  return typeof value === 'string' && UUID_PATTERN.test(value)
}

function isRevision(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isSafeInteger(value)
    && value >= 0
}

function isFoldModelFingerprint(value: unknown): value is string {
  return typeof value === 'string'
    && /^[0-9a-f]{64}$/u.test(value)
}

function isImmediateSourceLimitResult(
  job: GlobalFlatFoldabilityJobDto,
): boolean {
  return job.state === 'completed'
    && job.result.verdict === 'unknown'
    && job.result.reason === 'work_limit_reached'
}

function closedErrorCategory(
  value: unknown,
): GlobalFlatFoldabilityErrorCategory | null {
  try {
    if (
      value === null
      || typeof value !== 'object'
      || Array.isArray(value)
      || Object.getPrototypeOf(value) !== Object.prototype
    ) return null
    const descriptor = Object.getOwnPropertyDescriptor(value, 'category')
    if (
      descriptor === undefined
      || !('value' in descriptor)
      || !descriptor.enumerable
      || typeof descriptor.value !== 'string'
    ) return null
    switch (descriptor.value) {
      case 'invalid_request':
      case 'snapshot_unavailable':
      case 'worker_unavailable':
      case 'result_unavailable':
      case 'internal_failure':
        return descriptor.value
      default:
        return null
    }
  } catch {
    return null
  }
}

function exactDataRecord<const Keys extends readonly string[]>(
  value: unknown,
  expectedKeys: Keys,
): Readonly<Record<Keys[number], unknown>> | null {
  try {
    if (
      value === null
      || typeof value !== 'object'
      || Array.isArray(value)
      || Object.getPrototypeOf(value) !== Object.prototype
      || Object.getOwnPropertySymbols(value).length !== 0
    ) return null
    const descriptors = Object.getOwnPropertyDescriptors(value)
    const keys = Object.keys(descriptors)
    if (
      keys.length !== expectedKeys.length
      || expectedKeys.some((key) => !Object.hasOwn(descriptors, key))
    ) return null
    const result = Object.create(null) as Record<string, unknown>
    for (const key of expectedKeys) {
      const descriptor = descriptors[key]
      if (
        descriptor === undefined
        || !('value' in descriptor)
        || !descriptor.enumerable
      ) return null
      result[key] = descriptor.value
    }
    return result as Readonly<Record<Keys[number], unknown>>
  } catch {
    return null
  }
}
