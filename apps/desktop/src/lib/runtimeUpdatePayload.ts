import {
  isIssuedRuntimeUpdateAuthorization,
  type RuntimeUpdateAuthorization,
} from './runtimeUpdateManifest.ts'

const DEFAULT_MAX_PAYLOAD_BYTES = 512 * 1024 * 1024
const ABSOLUTE_MAX_PAYLOAD_BYTES = 1024 * 1024 * 1024

export type RuntimePayloadTransport = Readonly<{
  requestPayload: (assetName: string, signal?: AbortSignal) =>
    Promise<AsyncIterable<unknown>>
}>
export type RuntimePayloadSignatureVerifier = Readonly<{
  verifyPlatformSignature: (assetName: string, bytes: Uint8Array) =>
    Promise<boolean>
}>
export type RuntimePayloadStaging = Readonly<{
  begin: (assetName: string) => Promise<Readonly<{
    write: (chunk: Uint8Array) => Promise<void>
    commit: () => Promise<void>
    rollback: () => Promise<void>
  }>>
}>
export type RuntimePayloadResult =
  | Readonly<{ kind: 'staged'; assetName: string; byteLength: number }>
  | Readonly<{ kind: 'rejected'; reason: 'unauthorized' | 'network' | 'oversize' | 'hash_mismatch' | 'signature_mismatch' | 'storage' }>

/** Downloads only a parser-issued asset, verifies it, then atomically commits it. */
export async function stageAuthorizedRuntimePayload(
  authorization: RuntimeUpdateAuthorization,
  assetName: unknown,
  dependencies: Readonly<{
    transport: RuntimePayloadTransport
    signatureVerifier: RuntimePayloadSignatureVerifier
    staging: RuntimePayloadStaging
    maxPayloadBytes?: number
  }>,
  signal?: AbortSignal,
): Promise<RuntimePayloadResult> {
  if (!isIssuedRuntimeUpdateAuthorization(authorization) || typeof assetName !== 'string') {
    return rejected('unauthorized')
  }
  const asset = authorization.assets.find((candidate) => candidate.name === assetName)
  if (!asset) return rejected('unauthorized')
  const limit = dependencies.maxPayloadBytes ?? DEFAULT_MAX_PAYLOAD_BYTES
  if (!Number.isSafeInteger(limit) || limit <= 0 || limit > ABSOLUTE_MAX_PAYLOAD_BYTES) {
    return rejected('unauthorized')
  }

  let transaction: Awaited<ReturnType<RuntimePayloadStaging['begin']>>
  try { transaction = await dependencies.staging.begin(assetName) } catch { return rejected('storage') }
  let settled = false
  try {
    let stream: AsyncIterable<unknown>
    try { stream = await dependencies.transport.requestPayload(assetName, signal) } catch { return rejected('network') }
    const chunks: Uint8Array[] = []
    let byteLength = 0
    try {
      for await (const value of stream) {
        if (!(value instanceof Uint8Array) || value.byteLength === 0) return rejected('network')
        byteLength += value.byteLength
        if (!Number.isSafeInteger(byteLength) || byteLength > limit) return rejected('oversize')
        const chunk = value.slice()
        chunks.push(chunk)
        try { await transaction.write(chunk) } catch { return rejected('storage') }
      }
    } catch { return rejected('network') }
    const bytes = concatenate(chunks, byteLength)
    const digest = await sha256(bytes)
    if (digest !== asset.sha256) return rejected('hash_mismatch')
    let signatureValid = false
    try { signatureValid = await dependencies.signatureVerifier.verifyPlatformSignature(assetName, bytes) } catch { return rejected('signature_mismatch') }
    if (!signatureValid) return rejected('signature_mismatch')
    try { await transaction.commit() } catch { return rejected('storage') }
    settled = true
    return Object.freeze({ kind: 'staged', assetName, byteLength })
  } finally {
    if (!settled) {
      try { await transaction.rollback() } catch { /* fail closed; never expose staging authority */ }
    }
  }
}

function concatenate(chunks: readonly Uint8Array[], length: number): Uint8Array {
  const bytes = new Uint8Array(length)
  let offset = 0
  for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.byteLength }
  return bytes
}

async function sha256(bytes: Uint8Array): Promise<string> {
  const owned = new Uint8Array(new ArrayBuffer(bytes.byteLength))
  owned.set(bytes)
  const digest = await crypto.subtle.digest('SHA-256', owned.buffer)
  return [...new Uint8Array(digest)].map((value) => value.toString(16).padStart(2, '0')).join('')
}

function rejected(reason: Extract<RuntimePayloadResult, { kind: 'rejected' }>['reason']): RuntimePayloadResult {
  return Object.freeze({ kind: 'rejected', reason })
}
