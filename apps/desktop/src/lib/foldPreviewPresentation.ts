import type {
  FoldPreviewTreeSingleHingeCorrectionAnalysisCoordinatorState,
} from './foldPreviewTreeSingleHingeCorrectionAnalysisCoordinator.ts'
import type { FoldPreviewHingeAngle } from './foldPreviewKinematics.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
} from './i18n.ts'
import type { ResolvedLengthDisplayUnit } from './lengthUnit.ts'
import {
  FOLD_PREVIEW_PRESENTATION_INPUT as INPUT,
  FOLD_PREVIEW_PRESENTATION_TEXT as TEXT,
  formatFoldPreviewPresentationAngle,
  localizeFoldPreviewPaperEdgeRatioLength,
  type FoldPreviewRenderErrorCode,
} from './foldPreviewPresentationText.ts'

export type { FoldPreviewRenderErrorCode } from './foldPreviewPresentationText.ts'

export type FoldPreviewKeyboardAnnouncement =
  | Readonly<{
    kind: 'hinge_selected'
    index: number
    total: number
  }>
  | Readonly<{
    kind: 'fixed_face_selected'
    index: number
    total: number
  }>
  | Readonly<{
    kind: 'hinge_cleared'
  }>
  | Readonly<{
    kind: 'selection_changed'
  }>

export type FoldPreviewCorrectionAnalysisView = Readonly<{
  badgeText: string
  badgeClass: string
  accessibleText: string
  liveText: string
}>

export function describeFoldPreviewRenderError(
  code: FoldPreviewRenderErrorCode,
  locale: Locale,
): string {
  return selectLocalizedText(locale, TEXT.renderErrors[code])
}

/**
 * Converts the App-owned topology status into a small trusted presentation
 * vocabulary. Unknown or error-suffixed input is never copied into the UI.
 */
export function describeFoldPreviewStatus(
  value: unknown,
  locale: Locale,
): string {
  if (typeof value === 'string') {
    for (const message of TEXT.trustedStatuses) {
      if (value === message.ja || value === message.en) {
        return selectLocalizedText(locale, message)
      }
    }

    const faceCount = matchSafeCounts(value, [
      INPUT.statusFaceCountPatterns.ja,
      INPUT.statusFaceCountPatterns.en,
    ])
    if (faceCount) {
      const [faces, hinges] = faceCount
      return formatLocalizedText(
        locale,
        TEXT.statusFaceCount,
        {
          faces,
          hinges,
          faceNoun: selectLocalizedText(
            locale,
            faces === 1
              ? TEXT.statusFaceSingular
              : TEXT.statusFacePlural,
          ),
          hingeNoun: selectLocalizedText(
            locale,
            hinges === 1
              ? TEXT.statusHingeSingular
              : TEXT.statusHingePlural,
          ),
        },
      )
    }

    const blockedCount = matchSafeCounts(value, [
      INPUT.statusBlockedCountPatterns.ja,
      INPUT.statusBlockedCountPatterns.en,
    ])
    if (blockedCount) {
      const count = blockedCount[0]
      return formatLocalizedText(
        locale,
        TEXT.statusBlockedCount,
        {
          count,
          issueNoun: selectLocalizedText(
            locale,
            count === 1
              ? TEXT.statusIssueSingular
              : TEXT.statusIssuePlural,
          ),
        },
      )
    }

    if (
      value.startsWith(TEXT.statusAnalysisErrorPrefix.ja)
      || value.startsWith(TEXT.statusAnalysisErrorPrefix.en)
    ) {
      return selectLocalizedText(locale, TEXT.statusAnalysisFailed)
    }
  }
  return selectLocalizedText(locale, TEXT.statusWaiting)
}

export function describeFoldPreviewThickness(
  input: Readonly<{
    hasAuthoritativeThickness: boolean
    thicknessIsEmphasised: boolean
    thicknessIsLimited: boolean
    formattedLength: string
    lengthDisplayUnit: ResolvedLengthDisplayUnit
  }>,
  locale: Locale,
): string {
  const length = formatFoldPreviewLength(
    input.formattedLength,
    input.lengthDisplayUnit,
    locale,
  )
  if (!input.hasAuthoritativeThickness) {
    return formatLocalizedText(
      locale,
      TEXT.thicknessInvalid,
      { length },
    )
  }
  if (input.thicknessIsEmphasised) {
    return formatLocalizedText(
      locale,
      TEXT.thicknessEmphasised,
      { length },
    )
  }
  if (input.thicknessIsLimited) {
    return formatLocalizedText(
      locale,
      TEXT.thicknessLimited,
      { length },
    )
  }
  return formatLocalizedText(
    locale,
    TEXT.thicknessNormal,
    { length },
  )
}

export function describeFoldPreviewCorrectionAnalysis(
  state: FoldPreviewTreeSingleHingeCorrectionAnalysisCoordinatorState,
  locale: Locale,
): FoldPreviewCorrectionAnalysisView {
  switch (state.status) {
    case 'idle':
      return {
        badgeText: selectLocalizedText(locale, TEXT.correctionIdleBadge),
        badgeClass: 'is-idle',
        accessibleText: selectLocalizedText(
          locale,
          TEXT.correctionIdleAccessible,
        ),
        liveText: '',
      }
    case 'working': {
      const phaseText = correctionAnalysisPhaseText(state.phase, locale)
      return {
        badgeText: formatLocalizedText(
          locale,
          TEXT.correctionWorkingBadge,
          { phase: phaseText },
        ),
        badgeClass: 'is-working',
        accessibleText: formatLocalizedText(
          locale,
          TEXT.correctionWorkingAccessible,
          { phase: phaseText },
        ),
        liveText: selectLocalizedText(
          locale,
          TEXT.correctionWorkingLive,
        ),
      }
    }
    case 'stale':
      return {
        badgeText: selectLocalizedText(
          locale,
          TEXT.correctionStaleBadge,
        ),
        badgeClass: 'is-stale',
        accessibleText: selectLocalizedText(
          locale,
          TEXT.correctionStaleAccessible,
        ),
        liveText: selectLocalizedText(
          locale,
          TEXT.correctionStaleAccessible,
        ),
      }
    case 'no_candidate':
      return {
        badgeText: selectLocalizedText(
          locale,
          TEXT.correctionNoCandidateBadge,
        ),
        badgeClass: 'is-no-candidate',
        accessibleText: selectLocalizedText(
          locale,
          TEXT.correctionNoCandidateAccessible,
        ),
        liveText: selectLocalizedText(
          locale,
          TEXT.correctionNoCandidateLive,
        ),
      }
    case 'indeterminate':
      return {
        badgeText: selectLocalizedText(
          locale,
          TEXT.correctionIndeterminateBadge,
        ),
        badgeClass: 'is-indeterminate',
        accessibleText: selectLocalizedText(
          locale,
          TEXT.correctionIndeterminateAccessible,
        ),
        liveText: selectLocalizedText(
          locale,
          TEXT.correctionIndeterminateLive,
        ),
      }
    case 'certified':
      return certifiedCorrectionAnalysisView(state.presentation, locale)
  }
}

export function describeFoldPreviewTreeAngles(
  hingeAngles: readonly FoldPreviewHingeAngle[] | undefined,
  uniformAngle: number,
  locale: Locale,
): string {
  if (!hingeAngles || hingeAngles.length === 0) {
    const angle = formatFoldPreviewAngle(uniformAngle, locale)
    return formatLocalizedText(
      locale,
      TEXT.treeAnglesUniform,
      { angle },
    )
  }
  const values = hingeAngles.map(({ angleDegrees }) => angleDegrees)
  if (
    !values.every(
      (value) => Number.isFinite(value) && value >= 0 && value <= 180,
    )
  ) {
    return selectLocalizedText(locale, TEXT.treeAnglesPerHinge)
  }
  const minimum = Math.min(...values)
  const maximum = Math.max(...values)
  const minimumText = formatFoldPreviewAngle(minimum, locale)
  const maximumText = formatFoldPreviewAngle(maximum, locale)
  return minimum === maximum
    ? formatLocalizedText(
        locale,
        TEXT.treeAnglesAllHinges,
        { angle: minimumText },
      )
    : formatLocalizedText(
        locale,
        TEXT.treeAnglesRange,
        { minimum: minimumText, maximum: maximumText },
      )
}

export function formatFoldPreviewAngle(
  value: number,
  locale: Locale,
): string {
  return formatFoldPreviewPresentationAngle(value, locale)
}

export function normalizeFoldPreviewKeyboardAnnouncement(
  value: unknown,
): FoldPreviewKeyboardAnnouncement {
  if (value === INPUT.keyboardHingeCleared) {
    return Object.freeze({ kind: 'hinge_cleared' })
  }
  if (typeof value !== 'string') {
    return Object.freeze({ kind: 'selection_changed' })
  }
  const hingeMatch = INPUT.keyboardHingeSelectedPattern.exec(value)
  if (hingeMatch) {
    const counts = safeAnnouncementCounts(hingeMatch[1], hingeMatch[2])
    if (counts) {
      return Object.freeze({
        kind: 'hinge_selected',
        index: counts[0],
        total: counts[1],
      })
    }
  }
  const faceMatch = INPUT.keyboardFixedFaceSelectedPattern.exec(value)
  if (faceMatch) {
    const counts = safeAnnouncementCounts(faceMatch[1], faceMatch[2])
    if (counts) {
      return Object.freeze({
        kind: 'fixed_face_selected',
        index: counts[0],
        total: counts[1],
      })
    }
  }
  return Object.freeze({ kind: 'selection_changed' })
}

export function describeFoldPreviewKeyboardAnnouncement(
  announcement: FoldPreviewKeyboardAnnouncement,
  locale: Locale,
): string {
  switch (announcement.kind) {
    case 'hinge_selected':
      return formatLocalizedText(
        locale,
        TEXT.keyboardHingeSelected,
        { index: announcement.index, total: announcement.total },
      )
    case 'fixed_face_selected':
      return formatLocalizedText(
        locale,
        TEXT.keyboardFixedFaceSelected,
        { index: announcement.index, total: announcement.total },
      )
    case 'hinge_cleared':
      return selectLocalizedText(locale, TEXT.keyboardHingeCleared)
    case 'selection_changed':
      return selectLocalizedText(locale, TEXT.keyboardSelectionChanged)
  }
}

function correctionAnalysisPhaseText(
  phase: Extract<
    FoldPreviewTreeSingleHingeCorrectionAnalysisCoordinatorState,
    { status: 'working' }
  >['phase'],
  locale: Locale,
) {
  return selectLocalizedText(locale, TEXT.correctionPhases[phase])
}

function certifiedCorrectionAnalysisView(
  presentation: Extract<
    FoldPreviewTreeSingleHingeCorrectionAnalysisCoordinatorState,
    { status: 'certified' }
  >['presentation'],
  locale: Locale,
): FoldPreviewCorrectionAnalysisView {
  const rank = presentation.candidate.rank
  const source = presentation.angles.sourceDegrees
  const target = presentation.angles.targetDegrees
  const delta = presentation.angles.absoluteDeltaDegrees
  const expectedDelta = Math.abs(target - source)
  const expectedDirection = target > source ? 'increasing' : 'decreasing'
  if (
    !Number.isSafeInteger(rank)
    || rank < 1
    || !validAngle(source)
    || !validAngle(target)
    || source === target
    || !Number.isFinite(delta)
    || delta <= 0
    || delta > 180
    || delta !== expectedDelta
    || presentation.angles.deltaDegrees !== target - source
    || presentation.angles.direction !== expectedDirection
  ) {
    return {
      badgeText: selectLocalizedText(
        locale,
        TEXT.correctionIndeterminateBadge,
      ),
      badgeClass: 'is-indeterminate',
      accessibleText: selectLocalizedText(
        locale,
        TEXT.correctionInvalidCertifiedAccessible,
      ),
      liveText: selectLocalizedText(
        locale,
        TEXT.correctionInvalidCertifiedLive,
      ),
    }
  }

  const sourceText = preciseAngle(source)
  const targetText = preciseAngle(target)
  const deltaText = preciseAngle(delta)
  const direction = selectLocalizedText(
    locale,
    TEXT.correctionDirections[presentation.angles.direction],
  )
  const limitation = selectLocalizedText(
    locale,
    TEXT.correctionCertifiedLimitation,
  )
  const badgeText = formatLocalizedText(
    locale,
    TEXT.correctionCertifiedBadge,
    { rank, source: sourceText, target: targetText },
  )
  const accessibleText = formatLocalizedText(
    locale,
    TEXT.correctionCertifiedAccessible,
    {
      rank,
      source: sourceText,
      target: targetText,
      delta: deltaText,
      direction,
      limitation,
    },
  )
  return {
    badgeText,
    badgeClass: 'is-certified',
    accessibleText,
    liveText: accessibleText,
  }
}

function formatFoldPreviewLength(
  formatted: string,
  unit: ResolvedLengthDisplayUnit,
  locale: Locale,
) {
  return localizeFoldPreviewPaperEdgeRatioLength(
    formatted,
    unit.label,
    locale,
  )
}

function matchSafeCounts(
  value: string,
  expressions: readonly RegExp[],
): readonly number[] | null {
  for (const expression of expressions) {
    const match = expression.exec(value)
    if (!match) continue
    const counts = match.slice(1).map(Number)
    if (
      counts.every(
        (count) => Number.isSafeInteger(count) && count >= 0,
      )
    ) return counts
  }
  return null
}

function safeAnnouncementCounts(
  indexValue: string | undefined,
  totalValue: string | undefined,
): readonly [number, number] | null {
  const index = Number(indexValue)
  const total = Number(totalValue)
  return Number.isSafeInteger(index)
    && Number.isSafeInteger(total)
    && index > 0
    && total > 0
    && index <= total
    ? Object.freeze([index, total])
    : null
}

function validAngle(value: number) {
  return Number.isFinite(value) && value >= 0 && value <= 180
}

function preciseAngle(value: number) {
  const rounded = Math.round(value * 1_000) / 1_000
  return Object.is(rounded, -0) ? '0' : String(rounded)
}
