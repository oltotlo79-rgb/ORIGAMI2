/**
 * Formats a stored paper thickness for a number input without rounding it.
 *
 * Normal values always show at least hundredths of a millimetre, while values
 * entered with finer precision remain unchanged. Exponential notation is
 * expanded so very small finite values are still unambiguous in the input.
 */
export function formatPaperThicknessInput(
  value: number | null | undefined,
): string {
  if (
    typeof value !== 'number'
    || !Number.isFinite(value)
    || value < 0
  ) return ''

  const text = expandExponent(Object.is(value, -0) ? '0' : String(value))
  const decimalIndex = text.indexOf('.')
  if (decimalIndex < 0) return `${text}.00`
  const fractionDigits = text.length - decimalIndex - 1
  return fractionDigits >= 2
    ? text
    : `${text}${'0'.repeat(2 - fractionDigits)}`
}

export type PaperThicknessStepDirection = 'up' | 'down'

/**
 * Applies one exact decimal 0.01 mm step without snapping the current value to
 * a hundredth-of-a-millimetre grid.
 *
 * Keeping the operation in decimal form is important: native number-input
 * stepping first aligns values such as 0.075 to the step grid, while ordinary
 * binary floating-point addition can expose 0.08499999999999999. Empty,
 * malformed, and non-finite values are left for form validation to report.
 */
export function stepPaperThicknessInput(
  value: string,
  direction: PaperThicknessStepDirection,
): string {
  if (direction !== 'up' && direction !== 'down') return value
  const parsed = parseFiniteDecimal(value)
  if (!parsed) return value
  if (parsed.negative && parsed.coefficient !== 0n) return '0.00'

  const scale = Math.max(2, parsed.scale)
  const coefficient = parsed.coefficient
    * decimalPower(scale - parsed.scale)
  const step = decimalPower(scale - 2)
  const adjusted = direction === 'up'
    ? coefficient + step
    : coefficient - step

  return formatDecimalInteger(adjusted > 0n ? adjusted : 0n, scale)
}

type ParsedFiniteDecimal = {
  coefficient: bigint
  negative: boolean
  scale: number
}

const MAX_DECIMAL_SCALE = 400
const MAX_PAPER_THICKNESS_INPUT_CHARS = 512
const DECIMAL_INPUT_PATTERN =
  /^([+-]?)(?:(\d+)(?:\.(\d*))?|\.(\d+))(?:[eE]([+-]?\d+))?$/u

function parseFiniteDecimal(value: string): ParsedFiniteDecimal | null {
  if (value.length > MAX_PAPER_THICKNESS_INPUT_CHARS) return null
  const text = value.trim()
  if (!text || !Number.isFinite(Number(text))) return null

  const match = DECIMAL_INPUT_PATTERN.exec(text)
  if (!match) return null
  const integer = match[2] ?? '0'
  const fraction = match[3] ?? match[4] ?? ''
  const exponent = Number(match[5] ?? '0')
  if (!Number.isSafeInteger(exponent)) return null

  const scale = fraction.length - exponent
  if (
    scale < -MAX_DECIMAL_SCALE
    || scale > MAX_DECIMAL_SCALE
    || integer.length + fraction.length > MAX_DECIMAL_SCALE
  ) return null

  let coefficient = BigInt(`${integer}${fraction}`)
  let normalizedScale = scale
  if (normalizedScale < 0) {
    coefficient *= decimalPower(-normalizedScale)
    normalizedScale = 0
  }

  return {
    coefficient,
    negative: match[1] === '-',
    scale: normalizedScale,
  }
}

function decimalPower(exponent: number): bigint {
  return 10n ** BigInt(exponent)
}

function formatDecimalInteger(coefficient: bigint, scale: number): string {
  const digits = coefficient.toString().padStart(scale + 1, '0')
  if (scale === 0) return digits
  const decimalIndex = digits.length - scale
  return `${digits.slice(0, decimalIndex)}.${digits.slice(decimalIndex)}`
}

function expandExponent(value: string): string {
  const exponentIndex = value.search(/[eE]/u)
  if (exponentIndex < 0) return value

  const mantissa = value.slice(0, exponentIndex)
  const exponent = Number(value.slice(exponentIndex + 1))
  const [integer = '0', fraction = ''] = mantissa.split('.')
  const digits = `${integer}${fraction}`
  const decimalIndex = integer.length + exponent
  if (decimalIndex <= 0) {
    return `0.${'0'.repeat(-decimalIndex)}${digits}`
  }
  if (decimalIndex >= digits.length) {
    return `${digits}${'0'.repeat(decimalIndex - digits.length)}`
  }
  return `${digits.slice(0, decimalIndex)}.${digits.slice(decimalIndex)}`
}
