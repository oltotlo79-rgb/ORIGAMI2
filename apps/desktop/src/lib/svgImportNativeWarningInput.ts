import type {
  SvgImportWarningCategory,
  SvgImportWarningPresentationInput,
} from './svgImportPresentationText.ts'

type CountedSvgImportWarningCategory = Exclude<
  SvgImportWarningCategory,
  | 'invalid_title'
  | 'stroke_style_not_saved'
  | 'attributes_not_saved'
  | 'preview_omitted'
  | 'unknown'
>

export const SVG_IMPORT_NATIVE_WARNING_INPUTS = Object.freeze({
  fixed: Object.freeze({
    invalid_title:
      'SVG内のタイトルは作品名の条件に合わないため、既定の作品名を使用します。',
    stroke_style_not_saved:
      'SVGのstroke色、透明度、線幅、破線・線端表現は線種確認にだけ使用し、取込後には保存しません。',
    attributes_not_saved:
      'SVGのレイヤー、class、代表ID、data-origami-kindは線種確認にだけ使用し、取込後には保存しません。',
  }),
  countedPrefixes: Object.freeze([
    Object.freeze({
      prefix: '未対応の要素',
      category: 'unsupported_elements',
    }),
    Object.freeze({
      prefix: '未対応の属性',
      category: 'unsupported_attributes',
    }),
    Object.freeze({
      prefix: '未対応のstyle property',
      category: 'unsupported_style_properties',
    }),
    Object.freeze({
      prefix: '未対応のCSS selector',
      category: 'unsupported_css_selectors',
    }),
    Object.freeze({
      prefix: '曲線など未対応のpath command',
      category: 'unsupported_path_commands',
    }),
    Object.freeze({
      prefix: '未対応のstroke指定',
      category: 'unsupported_stroke_values',
    }),
    Object.freeze({
      prefix: '解決できない長さ指定',
      category: 'unresolved_lengths',
    }),
    Object.freeze({
      prefix: '外部参照',
      category: 'external_references',
    }),
    Object.freeze({
      prefix: '非表示の形状',
      category: 'hidden_shapes',
    }),
    Object.freeze({
      prefix: 'strokeのない形状',
      category: 'missing_stroke',
    }),
    Object.freeze({
      prefix: '塗り情報',
      category: 'fill_information',
    }),
    Object.freeze({
      prefix: 'SVG metadata',
      category: 'svg_metadata',
    }),
    Object.freeze({
      prefix: '空の形状',
      category: 'empty_shapes',
    }),
    Object.freeze({
      prefix: '物理寸法',
      category: 'physical_size',
    }),
    Object.freeze({
      prefix: 'CSSの96 px',
      category: 'css_pixels',
    }),
  ] satisfies ReadonlyArray<Readonly<{
    prefix: string
    category: CountedSvgImportWarningCategory
  }>>),
})

const SVG_IMPORT_FIXED_WARNING_CATEGORIES = Object.freeze(
  Object.keys(SVG_IMPORT_NATIVE_WARNING_INPUTS.fixed) as Array<
    keyof typeof SVG_IMPORT_NATIVE_WARNING_INPUTS.fixed
  >,
)
const SVG_IMPORT_PREVIEW_OMITTED =
  /^表示上限により([0-9]+)本の線をプレビューから省略しました。取込本体からは省略しません。$/u
const SVG_IMPORT_WARNING_COUNT = /（([0-9]+)件）。$/u

export function classifySvgImportNativeWarning(
  warning: unknown,
): SvgImportWarningPresentationInput | null {
  if (typeof warning !== 'string') return null

  for (const category of SVG_IMPORT_FIXED_WARNING_CATEGORIES) {
    if (warning === SVG_IMPORT_NATIVE_WARNING_INPUTS.fixed[category]) {
      return Object.freeze({ category })
    }
  }

  const omitted = SVG_IMPORT_PREVIEW_OMITTED.exec(warning)
  if (omitted) {
    return Object.freeze({
      category: 'preview_omitted',
      count: omitted[1] ?? '?',
    })
  }

  const counted = SVG_IMPORT_WARNING_COUNT.exec(warning)
  const count = counted?.[1]
  if (count) {
    for (
      const {
        prefix,
        category,
      } of SVG_IMPORT_NATIVE_WARNING_INPUTS.countedPrefixes
    ) {
      if (warning.startsWith(prefix)) {
        return Object.freeze({ category, count })
      }
    }
  }
  return null
}
