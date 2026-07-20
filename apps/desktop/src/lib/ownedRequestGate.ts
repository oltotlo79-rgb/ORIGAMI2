export type OwnedRequestGate = {
  active: boolean
  sequence: number
}

export function createOwnedRequestGate(): OwnedRequestGate {
  return { active: false, sequence: 0 }
}

export function tryBeginOwnedRequest(gate: OwnedRequestGate): number | null {
  if (gate.active) return null
  gate.sequence = gate.sequence >= 0xffff_ffff
    ? 1
    : gate.sequence + 1
  gate.active = true
  return gate.sequence
}

export function completeOwnedRequest(
  gate: OwnedRequestGate,
  requestId: number,
): boolean {
  if (!gate.active || gate.sequence !== requestId) return false
  gate.active = false
  return true
}

export function ownedRequestActive(gate: OwnedRequestGate): boolean {
  return gate.active
}
