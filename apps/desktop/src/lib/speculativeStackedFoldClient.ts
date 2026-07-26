import { invoke } from '@tauri-apps/api/core'
import { isCanonicalNonNilUuid } from './canonicalUuid.ts'

export type SpeculativeStackedFoldApplyRequestV1 = Readonly<{
  transactionToken: string
  explicitConfirmation: true
}>

export function normalizeSpeculativeStackedFoldApplyRequestV1(
  value: unknown,
): SpeculativeStackedFoldApplyRequestV1 | null {
  if (
    typeof value !== 'object'
    || value === null
    || Array.isArray(value)
  ) return null
  try {
    if (Object.getPrototypeOf(value) !== Object.prototype) return null
  } catch {
    return null
  }
  let keys: PropertyKey[]
  try {
    keys = Reflect.ownKeys(value)
  } catch {
    return null
  }
  if (
    keys.length !== 2
    || keys.some((key) =>
      typeof key !== 'string'
      || (
        key !== 'transactionToken'
        && key !== 'explicitConfirmation'
      ))
  ) return null
  const token = ownDataValue(value, 'transactionToken')
  const confirmation = ownDataValue(value, 'explicitConfirmation')
  if (
    !isCanonicalNonNilUuid(token)
    || confirmation !== true
  ) return null
  return Object.freeze({
    transactionToken: token,
    explicitConfirmation: true,
  })
}

export function applySpeculativeStackedFoldTransaction(
  request: SpeculativeStackedFoldApplyRequestV1,
): Promise<number> {
  const normalized = normalizeSpeculativeStackedFoldApplyRequestV1(request)
  if (!normalized) {
    return Promise.reject(new Error('invalid speculative stacked-fold apply request'))
  }
  return invoke<unknown>('apply_speculative_stacked_fold_transaction', {
    request: normalized,
  }).then((value) => {
    if (
      !Number.isSafeInteger(value)
      || Number(value) < 0
      || Object.is(value, -0)
    ) {
      throw new Error('invalid speculative stacked-fold apply response')
    }
    return value as number
  })
}

function ownDataValue(value: object, key: string): unknown {
  let descriptor: PropertyDescriptor | undefined
  try {
    descriptor = Object.getOwnPropertyDescriptor(value, key)
  } catch {
    return undefined
  }
  return descriptor
    && descriptor.enumerable
    && 'value' in descriptor
    ? descriptor.value
    : undefined
}
