import { APP_TEXT } from '../lib/appText.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
  type LocalizedText,
} from '../lib/i18n.ts'

function beginnerGeneratedInstructionLabel(
  locale: Locale,
  code: string,
): string {
  const text = (localized: LocalizedText) => (
    selectLocalizedText(locale, localized)
  )
  const axial =
    /^bounded_tree_river_axial_v1:([1-9][0-9]*(?:,[1-9][0-9]*)*)$/u
      .exec(code)
  if (axial) {
    return formatLocalizedText(
      locale,
      APP_TEXT.useBoundedTreeRiverAxialRatios,
      { ratios: axial[1] ?? '' },
    )
  }
  const radialCornerSupport =
    /^bounded_radial_corner_support_v1:added=([0-5]):covered=4$/u.exec(code)
  if (radialCornerSupport) {
    return formatLocalizedText(
      locale,
      APP_TEXT.useBoundedRadialCornerSupport,
      { added: radialCornerSupport[1] ?? '' },
    )
  }
  const topology =
    /^bounded_tree_branch_topology_v1:nodes=([1-9][0-9]*):leaves=([1-9][0-9]*):bars=([1-9][0-9]*)$/u
      .exec(code)
  if (topology) {
    return formatLocalizedText(
      locale,
      APP_TEXT.useBoundedTreeBranchTopology,
      {
        nodes: topology[1] ?? '',
        leaves: topology[2] ?? '',
        bars: topology[3] ?? '',
      },
    )
  }
  if (code === 'bounded_tree_paper_orientation_v1:horizontal') {
    return text(APP_TEXT.useBoundedTreePaperOrientationHorizontal)
  }
  if (code === 'bounded_tree_paper_orientation_v1:vertical') {
    return text(APP_TEXT.useBoundedTreePaperOrientationVertical)
  }
  switch (code) {
    case 'symmetric_four_leg_base':
      return text(APP_TEXT.createTheSymmetricFourLegBaseFromTheSharedCenter)
    case 'symmetric_wing_base':
      return text(APP_TEXT.createTheBilateralWingBaseFromTheSharedCenter)
    case 'symmetric_bird_base':
      return text(APP_TEXT.createTheBilateralBirdWingBase)
    case 'asymmetric_bird_landmark_base':
      return text(
        APP_TEXT.createTheAsymmetricBirdBaseBoundToIndividualLandmarks,
      )
    case 'asymmetric_four_leg_landmark_base':
      return text(
        APP_TEXT.createTheAsymmetricFourLegBaseBoundToFourIndividual,
      )
    case 'asymmetric_insect_landmark_base':
      return text(APP_TEXT.bindTenOrderedInsectLandmarksToTheCertifiedFourRay)
    case 'asymmetric_fish_landmark_base':
      return text(APP_TEXT.bindTheHeadTailAndLeftRightFinsToThe)
    case 'symmetric_fish_base':
      return text(APP_TEXT.createTheBilateralFishFinBase)
    case 'symmetric_ear_base':
      return text(APP_TEXT.createTheBilateralLongEarBase)
    case 'symmetric_horn_base':
      return text(APP_TEXT.createTheBilateralHornBase)
    case 'symmetric_antenna_base':
      return text(APP_TEXT.createTheBilateralInsectAntennaBase)
    case 'symmetric_insect_leg_pair_base':
      return text(APP_TEXT.createOneBilateralInsectLegPairBase)
    case 'symmetric_six_leg_base':
      return text(APP_TEXT.createTheSymmetricCompleteSixLegBase)
    case 'center_axis_tail_base':
      return text(APP_TEXT.createTheCenterAxisTailBase)
    case 'center_axis_horn_base':
      return text(APP_TEXT.createTheCenterAxisSingleHornBase)
    case 'center_axis_antenna_base':
      return text(APP_TEXT.createTheCenterAxisSingleAntennaBase)
    case 'composite_tail_ear_base':
      return text(APP_TEXT.createTheCompositeTailAndEarBase)
    case 'composite_horn_ear_base':
      return text(APP_TEXT.createTheCompositeHornAndEarBase)
    case 'composite_horn_tail_base':
      return text(APP_TEXT.createTheCompositeHornAndTailBase)
    case 'composite_horn_tail_ear_base':
      return text(APP_TEXT.createTheCompositeHornTailAndEarBase)
    case 'composite_wing_antenna_base':
      return text(APP_TEXT.createTheCompositeWingAndAntennaBase)
    case 'composite_complete_insect_base':
      return text(APP_TEXT.createTheCompleteCompositeInsectBase)
    case 'composite_complete_animal_base':
      return text(APP_TEXT.createTheCompleteCompositeAnimalBase)
    case 'composite_complete_winged_animal_base':
      return text(APP_TEXT.createTheCompleteCompositeWingedAnimalBase)
    case 'book_fold_vertical':
      return text(APP_TEXT.foldInHalfOnTheVerticalCenterLine)
    case 'book_fold_horizontal':
      return text(APP_TEXT.foldInHalfOnTheHorizontalCenterLine)
    case 'diagonal_fold':
      return text(APP_TEXT.foldOnTheDiagonal)
    default:
      return formatLocalizedText(
        locale,
        APP_TEXT.unknownGeneratedInstructionCode,
        { code },
      )
  }
}

export function BeginnerGeneratedInstructionList({
  locale,
  instructionCodes,
}: Readonly<{
  locale: Locale
  instructionCodes: readonly string[]
}>) {
  if (instructionCodes.length === 0) {
    return (
      <p role="status" aria-live="polite" aria-atomic="true">
        {selectLocalizedText(
          locale,
          APP_TEXT.noValidatedCandidateFoldingInstructions,
        )}
      </p>
    )
  }
  return (
    <ol
      aria-label={selectLocalizedText(
        locale,
        APP_TEXT.candidateFoldingInstructions,
      )}
    >
      {instructionCodes.map((code, index) => (
        <li key={`${index}:${code}`}>
          {beginnerGeneratedInstructionLabel(locale, code)}
        </li>
      ))}
    </ol>
  )
}
