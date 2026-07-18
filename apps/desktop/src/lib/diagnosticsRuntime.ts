import { invoke } from '@tauri-apps/api/core'

import {
  DIAGNOSTIC_SCOPES,
  reportUnexpected as reportUnexpectedInMemory,
  type DiagnosticScope,
} from './diagnostics.ts'

export type DiagnosticsRuntimeDependencies = Readonly<{
  reportInMemory: (scope: DiagnosticScope) => boolean
  isNativeAvailable: () => boolean
  recordNative: (scope: DiagnosticScope) => unknown
}>

export type DiagnosticsRuntime = Readonly<{
  reportUnexpected: (scope: DiagnosticScope) => boolean
}>

const MAX_NATIVE_DELIVERIES_PER_SCOPE = 65
const SCOPE_INDEX = new Map<DiagnosticScope, number>(
  DIAGNOSTIC_SCOPES.map((scope, index) => [scope, index]),
)

export function createDiagnosticsRuntime(
  dependencies: DiagnosticsRuntimeDependencies,
): DiagnosticsRuntime {
  const nativeDeliveries = new Uint8Array(DIAGNOSTIC_SCOPES.length)

  return Object.freeze({
    reportUnexpected: (scope: DiagnosticScope) => {
      let accepted = false
      try {
        accepted = dependencies.reportInMemory(scope)
      } catch {
        return false
      }
      if (!accepted || typeof scope !== 'string') return false

      const scopeIndex = SCOPE_INDEX.get(scope)
      if (scopeIndex === undefined) return false

      try {
        if (!dependencies.isNativeAvailable()) return true
      } catch {
        return true
      }

      if (
        (nativeDeliveries[scopeIndex] ?? 0)
          >= MAX_NATIVE_DELIVERIES_PER_SCOPE
      ) {
        return true
      }
      nativeDeliveries[scopeIndex] += 1

      try {
        const delivery = dependencies.recordNative(scope)
        void Promise.resolve(delivery).catch(() => undefined)
      } catch {
        // The bounded in-memory record remains available to the current session.
      }
      return true
    },
  })
}

const applicationDiagnosticsRuntime = createDiagnosticsRuntime({
  reportInMemory: reportUnexpectedInMemory,
  isNativeAvailable: () =>
    typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window,
  recordNative: (scope) =>
    invoke('record_unexpected_diagnostic', { scope }),
})

export function reportUnexpected(scope: DiagnosticScope) {
  return applicationDiagnosticsRuntime.reportUnexpected(scope)
}
