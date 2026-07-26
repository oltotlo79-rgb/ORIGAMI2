import {
  formatLocalizedText,
  selectLocalizedText,
  type LocalizedText,
} from './i18n.ts'
import {
  classifySvgImportNativeWarning,
} from './svgImportNativeWarningInput.ts'
import type {
  SvgImportStyleGroup,
  SvgImportTarget,
} from './svgImport.ts'

export type SvgImportWarningCategory =
  | 'invalid_title'
  | 'stroke_style_not_saved'
  | 'attributes_not_saved'
  | 'unsupported_elements'
  | 'unsupported_attributes'
  | 'unsupported_style_properties'
  | 'unsupported_css_selectors'
  | 'unsupported_path_commands'
  | 'unsupported_stroke_values'
  | 'unresolved_lengths'
  | 'external_references'
  | 'hidden_shapes'
  | 'missing_stroke'
  | 'fill_information'
  | 'svg_metadata'
  | 'empty_shapes'
  | 'physical_size'
  | 'css_pixels'
  | 'preview_omitted'
  | 'unknown'

export type SvgImportWarningPresentationInput = Readonly<{
  category: SvgImportWarningCategory
  count?: string
}>

export const SVG_IMPORT_PRESENTATION_TEXT = Object.freeze({
  targetLabels: Object.freeze({
    boundary: localized('用紙境界', 'Paper boundary'),
    mountain: localized('山折り', 'Mountain fold'),
    valley: localized('谷折り', 'Valley fold'),
    auxiliary: localized('補助線', 'Auxiliary line'),
    cut: localized('切断線', 'Cut line'),
    ignore: localized('取り込まない', 'Do not import'),
  }),
  styleLabels: Object.freeze({
    layer: localized('レイヤー: {value}', 'Layer: {value}'),
    className: localized('class: {value}', 'class: {value}'),
    representativeId: localized(
      '代表ID: {value}',
      'Representative ID: {value}',
    ),
    semanticHint: localized(
      '属性: data-origami-kind={value}',
      'Attribute: data-origami-kind={value}',
    ),
    color: localized('色: {value}', 'Color: {value}'),
    dashPattern: localized('線種: {value}', 'Dash pattern: {value}'),
    lineCap: localized('線端: {value}', 'Line cap: {value}'),
    unknown: localized('不明', 'Unknown'),
    noStyleAttributes: localized('属性指定なし', 'No style attributes'),
    partSeparator: localized(' / ', ' / '),
  }),
  warningMessages: Object.freeze({
    invalid_title: localized(
      'SVG内のタイトルは作品名の条件に合わないため、既定の作品名を使用します。',
      'The SVG title does not meet the project-name requirements, so the default project name will be used.',
    ),
    stroke_style_not_saved: localized(
      'SVGのstroke色、透明度、線幅、破線・線端表現は線種確認にだけ使用し、取込後には保存しません。',
      'SVG stroke color, opacity, width, dash, and line-cap styling are used only to review line types and will not be saved after import.',
    ),
    attributes_not_saved: localized(
      'SVGのレイヤー、class、代表ID、data-origami-kindは線種確認にだけ使用し、取込後には保存しません。',
      'SVG layers, classes, representative IDs, and data-origami-kind attributes are used only to review line types and will not be saved after import.',
    ),
    unsupported_elements: warningCount(
      'Unsupported SVG elements were excluded',
    ),
    unsupported_attributes: warningCount(
      'Unsupported SVG attributes were ignored',
    ),
    unsupported_style_properties: warningCount(
      'Unsupported SVG style properties were ignored',
    ),
    unsupported_css_selectors: warningCount(
      'Unsupported CSS selectors were ignored',
    ),
    unsupported_path_commands: warningCount(
      'Paths with unsupported commands were excluded',
    ),
    unsupported_stroke_values: warningCount(
      'Lines with unsupported stroke values were excluded',
    ),
    unresolved_lengths: warningCount(
      'Shapes with unresolved length values were excluded',
    ),
    external_references: warningCount(
      'External references were not fetched and were excluded',
    ),
    hidden_shapes: warningCount('Hidden shapes were excluded'),
    missing_stroke: warningCount('Shapes without a stroke were excluded'),
    fill_information: warningCount('Fill information will not be saved'),
    svg_metadata: warningCount('SVG metadata will not be saved'),
    empty_shapes: warningCount('Empty shapes were excluded'),
    physical_size: warningCount(
      'A scale must be entered because the physical size is ambiguous',
    ),
    css_pixels: warningCount(
      'The CSS conversion of 96 px per inch was used and may differ from the author’s intent',
    ),
    preview_omitted: localized(
      '表示上限により{count}本の線をプレビューから省略しました。取込本体からは省略しません。',
      '{count} lines were omitted from the preview display limit. They will still be imported.',
    ),
    unknown: localized(
      'SVGの一部の情報は取り込まれないか変更されます。',
      'Some SVG information will not be imported or will be changed.',
    ),
  }),
}) satisfies Readonly<{
  targetLabels: Readonly<Record<SvgImportTarget, LocalizedText>>
  styleLabels: Readonly<Record<
    | 'layer'
    | 'className'
    | 'representativeId'
    | 'semanticHint'
    | 'color'
    | 'dashPattern'
    | 'lineCap'
    | 'unknown'
    | 'noStyleAttributes'
    | 'partSeparator',
    LocalizedText
  >>
  warningMessages: Readonly<
    Record<SvgImportWarningCategory, LocalizedText>
  >
}>

const FIXED_ENGLISH_WARNING_CATEGORIES =
  new Set<SvgImportWarningCategory>([
    'invalid_title',
    'stroke_style_not_saved',
    'attributes_not_saved',
  ])

export function svgImportTargetLabel(
  target: SvgImportTarget,
  locale: unknown,
): string {
  return selectLocalizedText(
    locale,
    SVG_IMPORT_PRESENTATION_TEXT.targetLabels[target],
  )
}

export function formatSvgImportStylePresentation(
  group: SvgImportStyleGroup,
  hasKnownLineCap: boolean,
  locale: unknown,
): string {
  const copy = SVG_IMPORT_PRESENTATION_TEXT.styleLabels
  const parts: string[] = []
  if (group.layer) {
    parts.push(formatLocalizedText(locale, copy.layer, {
      value: group.layer,
    }))
  }
  if (group.classes.length > 0) {
    parts.push(formatLocalizedText(locale, copy.className, {
      value: group.classes.join(' '),
    }))
  }
  if (group.representative_id) {
    parts.push(formatLocalizedText(locale, copy.representativeId, {
      value: group.representative_id,
    }))
  }
  if (group.semantic_hint) {
    parts.push(formatLocalizedText(locale, copy.semanticHint, {
      value: group.semantic_hint,
    }))
  }
  if (group.stroke) {
    parts.push(formatLocalizedText(locale, copy.color, {
      value: group.stroke,
    }))
  }
  if (group.dash_array) {
    parts.push(formatLocalizedText(locale, copy.dashPattern, {
      value: group.dash_array,
    }))
  }
  parts.push(formatLocalizedText(locale, copy.lineCap, {
    value: hasKnownLineCap
      ? group.line_cap
      : selectLocalizedText(locale, copy.unknown),
  }))
  return parts.length > 0
    ? parts.join(selectLocalizedText(locale, copy.partSeparator))
    : selectLocalizedText(locale, copy.noStyleAttributes)
}

export function formatSvgImportWarningPresentation(
  warning: string,
  locale: unknown,
): string {
  if (locale === 'ja') return warning
  const input = classifySvgImportNativeWarning(warning)
  const message =
    SVG_IMPORT_PRESENTATION_TEXT.warningMessages[input?.category ?? 'unknown']
  if (
    input
    && FIXED_ENGLISH_WARNING_CATEGORIES.has(input.category)
  ) {
    return message.en
  }
  return formatLocalizedText(
    locale,
    message,
    input?.count === undefined ? undefined : { count: input.count },
  )
}

function warningCount(en: string): LocalizedText {
  return localized('{count}件', `${en} ({count} occurrences).`)
}

function localized(ja: string, en: string): LocalizedText {
  return Object.freeze({ ja, en })
}
