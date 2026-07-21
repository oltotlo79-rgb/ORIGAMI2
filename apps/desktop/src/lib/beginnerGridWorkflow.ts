export type BeginnerGridApplyWorkflow = {
  confirm: () => boolean
  apply: () => Promise<boolean>
  clearPreview: () => void
  restoreFocus: () => void
}

export async function runBeginnerGridApplyWorkflow({
  confirm,
  apply,
  clearPreview,
  restoreFocus,
}: BeginnerGridApplyWorkflow) {
  if (!confirm()) return false
  if (!await apply()) return false
  clearPreview()
  restoreFocus()
  return true
}

export function finishBeginnerGridCancellation(
  clearPreview: () => void,
  restoreFocus: () => void,
) {
  clearPreview()
  restoreFocus()
}
