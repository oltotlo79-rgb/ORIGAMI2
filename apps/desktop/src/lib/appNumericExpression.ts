import { APP_TEXT } from './appText.ts'
import { selectLocalizedText, type Locale } from './i18n'
import type { ResolvedLengthDisplayUnit } from './lengthUnit'
import {
  evaluateFiniteNumericExpression,
  numericExpressionNativeErrorCategory,
} from './numericExpressionNative'

export function newProjectExpressionErrorMessage(
  error: unknown,
  locale: Locale,
) {
  const category = numericExpressionNativeErrorCategory(error)
  if (!category) return null
  switch (category) {
    case 'invalid_request':
      return selectLocalizedText(locale, APP_TEXT.theWidthOrHeightExpressionIsEmptyOrExceedsAn)
    case 'invalid_expression':
      return selectLocalizedText(locale, APP_TEXT.theWidthOrHeightExpressionCouldNotBeParsed)
    case 'resource_limit':
      return selectLocalizedText(locale, APP_TEXT.evaluationStoppedBecauseTheWidthOrHeightExpressionIsToo)
    case 'result_out_of_range':
      return selectLocalizedText(locale, APP_TEXT.theWidthOrHeightCannotBeSafelyUsedAsA)
    case 'native_unavailable':
      return selectLocalizedText(locale, APP_TEXT.creatingAProjectFromExpressionsIsAvailableInTheDesktop)
    case 'invalid_response':
    case 'stale_response':
    case 'internal_failure':
      return selectLocalizedText(locale, APP_TEXT.theEvaluatedWidthOrHeightResultCouldNotBeUsed)
  }
}

export async function evaluateDisplayLengthExpression(
  source: string,
  unit: ResolvedLengthDisplayUnit,
) {
  const adopted = await evaluateFiniteNumericExpression(source)
  const millimetres = adopted.value * unit.millimetresPerUnit
  if (!Number.isFinite(millimetres)) {
    throw new Error('display length expression overflow')
  }
  return millimetres === 0 ? 0 : millimetres
}

export function millimetreExpressionSource(
  source: string,
  millimetresPerUnit: number,
) {
  if (millimetresPerUnit === 1) return source
  return `(${source}) * ${finiteNumberExpressionSource(millimetresPerUnit)}`
}

export function finiteNumberExpressionSource(value: number) {
  if (!Number.isFinite(value)) throw new Error('non-finite expression source')
  return String(value === 0 ? 0 : value)
}
