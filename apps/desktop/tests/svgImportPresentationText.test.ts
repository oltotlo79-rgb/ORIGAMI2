import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

import {
  localizedSvgImportTargetOptions,
  SVG_IMPORT_TARGET_OPTIONS,
  svgImportStyleLabel,
  svgImportWarningText,
  type SvgImportStyleGroup,
} from '../src/lib/svgImport.ts'
import {
  classifySvgImportNativeWarning,
  SVG_IMPORT_NATIVE_WARNING_INPUTS,
} from '../src/lib/svgImportNativeWarningInput.ts'
import {
  SVG_IMPORT_PRESENTATION_TEXT as TEXT,
} from '../src/lib/svgImportPresentationText.ts'
import type { Locale } from '../src/lib/i18n.ts'

const ROOT_KEYS = [
  'targetLabels',
  'styleLabels',
  'warningMessages',
] as const

const TARGET_KEYS = [
  'boundary',
  'mountain',
  'valley',
  'auxiliary',
  'cut',
  'ignore',
] as const

const STYLE_KEYS = [
  'layer',
  'className',
  'representativeId',
  'semanticHint',
  'color',
  'dashPattern',
  'lineCap',
  'unknown',
  'noStyleAttributes',
  'partSeparator',
] as const

const WARNING_KEYS = [
  'invalid_title',
  'stroke_style_not_saved',
  'attributes_not_saved',
  'unsupported_elements',
  'unsupported_attributes',
  'unsupported_style_properties',
  'unsupported_css_selectors',
  'unsupported_path_commands',
  'unsupported_stroke_values',
  'unresolved_lengths',
  'external_references',
  'hidden_shapes',
  'missing_stroke',
  'fill_information',
  'svg_metadata',
  'empty_shapes',
  'physical_size',
  'css_pixels',
  'preview_omitted',
  'unknown',
] as const

test('SVG import presentation catalog is exact, complete, and deeply frozen', () => {
  assert.deepEqual(Object.keys(TEXT), ROOT_KEYS)
  assert.deepEqual(Object.keys(TEXT.targetLabels), TARGET_KEYS)
  assert.deepEqual(Object.keys(TEXT.styleLabels), STYLE_KEYS)
  assert.deepEqual(Object.keys(TEXT.warningMessages), WARNING_KEYS)
  assertDeepFrozen(TEXT)
  assert.equal(assertLocalizedLeaves(TEXT), 36)
  assert.equal(
    createHash('sha256')
      .update(JSON.stringify(TEXT), 'utf8')
      .digest('hex'),
    '50b5c0e9dfff97a9b15d7c4639e51cd80e681e4360f2d87929c0896bb3c30c04',
  )
})

test('target option order and every Japanese and English label stay stable', () => {
  assert.deepEqual(
    SVG_IMPORT_TARGET_OPTIONS,
    [
      { value: 'boundary', label: '用紙境界' },
      { value: 'mountain', label: '山折り' },
      { value: 'valley', label: '谷折り' },
      { value: 'auxiliary', label: '補助線' },
      { value: 'cut', label: '切断線' },
      { value: 'ignore', label: '取り込まない' },
    ],
  )
  assert.deepEqual(
    localizedSvgImportTargetOptions(null, 'en'),
    [
      { value: 'boundary', label: 'Paper boundary' },
      { value: 'mountain', label: 'Mountain fold' },
      { value: 'valley', label: 'Valley fold' },
      { value: 'auxiliary', label: 'Auxiliary line' },
      { value: 'cut', label: 'Cut line' },
      { value: 'ignore', label: 'Do not import' },
    ],
  )
  assert.deepEqual(
    localizedSvgImportTargetOptions(7, 'fr' as Locale),
    [
      { value: 'mountain', label: '山折り' },
      { value: 'valley', label: '谷折り' },
      { value: 'auxiliary', label: '補助線' },
      { value: 'cut', label: '切断線' },
      { value: 'ignore', label: '取り込まない' },
    ],
  )
})

test('style presentation preserves ordering, raw values, and invalid line-cap fallback', () => {
  const group: SvgImportStyleGroup = {
    group_id: 7,
    element_count: 2,
    segment_count: 3,
    stroke: '#123456',
    stroke_color: '#123456',
    dash_array: '4 2',
    line_cap: 'triangle' as never,
    classes: ['fold', 'selected'],
    layer: '外周',
    representative_id: 'paper-outline',
    semantic_hint: 'boundary',
  }
  assert.equal(
    svgImportStyleLabel(group, 'ja'),
    'レイヤー: 外周 / class: fold selected / 代表ID: paper-outline / 属性: data-origami-kind=boundary / 色: #123456 / 線種: 4 2 / 線端: 不明',
  )
  assert.equal(
    svgImportStyleLabel(group, 'en'),
    'Layer: 外周 / class: fold selected / Representative ID: paper-outline / Attribute: data-origami-kind=boundary / Color: #123456 / Dash pattern: 4 2 / Line cap: Unknown',
  )
  assert.equal(
    svgImportStyleLabel(group, 'fr' as Locale),
    svgImportStyleLabel(group, 'ja'),
  )
})

test('all fixed and counted native warning categories preserve exact output', () => {
  const fixedCases = [
    [
      SVG_IMPORT_NATIVE_WARNING_INPUTS.fixed.invalid_title,
      'The SVG title does not meet the project-name requirements, so the default project name will be used.',
    ],
    [
      SVG_IMPORT_NATIVE_WARNING_INPUTS.fixed.stroke_style_not_saved,
      'SVG stroke color, opacity, width, dash, and line-cap styling are used only to review line types and will not be saved after import.',
    ],
    [
      SVG_IMPORT_NATIVE_WARNING_INPUTS.fixed.attributes_not_saved,
      'SVG layers, classes, representative IDs, and data-origami-kind attributes are used only to review line types and will not be saved after import.',
    ],
  ] as const
  for (const [warning, english] of fixedCases) {
    assert.equal(svgImportWarningText(warning, 'ja'), warning)
    assert.equal(svgImportWarningText(warning, 'en'), english)
  }

  const countedCases = [
    ['未対応の要素', 'Unsupported SVG elements were excluded'],
    ['未対応の属性', 'Unsupported SVG attributes were ignored'],
    [
      '未対応のstyle property',
      'Unsupported SVG style properties were ignored',
    ],
    ['未対応のCSS selector', 'Unsupported CSS selectors were ignored'],
    [
      '曲線など未対応のpath command',
      'Paths with unsupported commands were excluded',
    ],
    [
      '未対応のstroke指定',
      'Lines with unsupported stroke values were excluded',
    ],
    [
      '解決できない長さ指定',
      'Shapes with unresolved length values were excluded',
    ],
    [
      '外部参照',
      'External references were not fetched and were excluded',
    ],
    ['非表示の形状', 'Hidden shapes were excluded'],
    ['strokeのない形状', 'Shapes without a stroke were excluded'],
    ['塗り情報', 'Fill information will not be saved'],
    ['SVG metadata', 'SVG metadata will not be saved'],
    ['空の形状', 'Empty shapes were excluded'],
    [
      '物理寸法',
      'A scale must be entered because the physical size is ambiguous',
    ],
    [
      'CSSの96 px',
      'The CSS conversion of 96 px per inch was used and may differ from the author’s intent',
    ],
  ] as const
  for (const [prefix, english] of countedCases) {
    const warning = `${prefix}の詳細（00042件）。`
    assert.equal(svgImportWarningText(warning, 'ja'), warning)
    assert.equal(
      svgImportWarningText(warning, 'en'),
      `${english} (00042 occurrences).`,
    )
  }
})

test('preview and count classifiers preserve digit strings and exact regex boundaries', () => {
  const count = '000000000000000000000000000000000000000042'
  const preview =
    `表示上限により${count}本の線をプレビューから省略しました。取込本体からは省略しません。`
  assert.deepEqual(
    classifySvgImportNativeWarning(preview),
    { category: 'preview_omitted', count },
  )
  assert.equal(
    svgImportWarningText(preview, 'en'),
    `${count} lines were omitted from the preview display limit. They will still be imported.`,
  )

  const counted = `未対応の要素\nprivate detail（${count}件）。`
  assert.deepEqual(
    classifySvgImportNativeWarning(counted),
    { category: 'unsupported_elements', count },
  )
  assert.equal(
    svgImportWarningText(counted, 'en'),
    `Unsupported SVG elements were excluded (${count} occurrences).`,
  )

  for (const malformed of [
    `${preview}suffix`,
    preview.replace('。取込', '\n取込'),
    `未対応の要素（${count}件）`,
    `未対応の要素(${count}件)。`,
    '未対応の要素（件）。',
    `unknown prefix（${count}件）。`,
  ]) {
    assert.equal(classifySvgImportNativeWarning(malformed), null)
    assert.equal(
      svgImportWarningText(malformed, 'en'),
      'Some SVG information will not be imported or will be changed.',
    )
  }
})

test('unknown and hostile warnings cannot be reflected into English output', () => {
  const privatePath = String.raw`C:\Users\alice\秘密\private.svg`
  const hostile = new Proxy(Object.create(null) as object, {
    get() {
      throw new Error('must not inspect hostile warning properties')
    },
    ownKeys() {
      throw new Error('must not enumerate hostile warning properties')
    },
  })
  const invoke = svgImportWarningText as unknown as (
    warning: unknown,
    locale: Locale,
  ) => unknown

  for (const warning of [
    privatePath,
    `unknown ${privatePath}（9件）。`,
    hostile,
    Symbol(privatePath),
    null,
  ]) {
    const output = invoke(warning, 'en')
    assert.equal(
      output,
      'Some SVG information will not be imported or will be changed.',
    )
    assert.doesNotMatch(String(output), /alice|private|秘密/u)
  }
  assert.equal(svgImportWarningText(privatePath, 'ja'), privatePath)
})

test('unknown and hostile locale values retain the legacy warning behavior', () => {
  const hostileLocale = new Proxy(Object.create(null) as object, {
    get() {
      throw new Error('must not read hostile locale properties')
    },
    getOwnPropertyDescriptor() {
      throw new Error('must not inspect hostile locale properties')
    },
  }) as Locale
  const unknownLocale = 'fr' as Locale

  for (const locale of [unknownLocale, hostileLocale]) {
    assert.equal(
      svgImportWarningText(
        SVG_IMPORT_NATIVE_WARNING_INPUTS.fixed.invalid_title,
        locale,
      ),
      'The SVG title does not meet the project-name requirements, so the default project name will be used.',
    )
    assert.equal(
      svgImportWarningText(
        '表示上限により007本の線をプレビューから省略しました。取込本体からは省略しません。',
        locale,
      ),
      '表示上限により007本の線をプレビューから省略しました。取込本体からは省略しません。',
    )
    assert.equal(
      svgImportWarningText('未対応の要素「foreignObject」を除外（007件）。', locale),
      '007件',
    )
    assert.equal(
      svgImportWarningText('private diagnostic', locale),
      'SVGの一部の情報は取り込まれないか変更されます。',
    )
  }
})

test('native warning inputs are closed and deeply frozen in matching order', () => {
  assert.deepEqual(
    Object.keys(SVG_IMPORT_NATIVE_WARNING_INPUTS.fixed),
    WARNING_KEYS.slice(0, 3),
  )
  for (
    const category of Object.keys(
      SVG_IMPORT_NATIVE_WARNING_INPUTS.fixed,
    ) as Array<keyof typeof SVG_IMPORT_NATIVE_WARNING_INPUTS.fixed>
  ) {
    assert.equal(
      SVG_IMPORT_NATIVE_WARNING_INPUTS.fixed[category],
      TEXT.warningMessages[category].ja,
    )
  }
  assert.deepEqual(
    SVG_IMPORT_NATIVE_WARNING_INPUTS.countedPrefixes.map(
      ({ category }) => category,
    ),
    WARNING_KEYS.slice(3, 18),
  )
  assertDeepFrozen(SVG_IMPORT_NATIVE_WARNING_INPUTS)
})

test('svgImport delegates presentation and native-warning interpretation', async () => {
  const [consumerSource, nativeInputSource, presentationSource] =
    await Promise.all([
      readFile(new URL('../src/lib/svgImport.ts', import.meta.url), 'utf8'),
      readFile(
        new URL('../src/lib/svgImportNativeWarningInput.ts', import.meta.url),
        'utf8',
      ),
      readFile(
        new URL('../src/lib/svgImportPresentationText.ts', import.meta.url),
        'utf8',
      ),
    ])

  assert.match(consumerSource, /SVG_IMPORT_PRESENTATION_TEXT/u)
  assert.match(consumerSource, /svgImportTargetLabel\(value, locale\)/u)
  assert.match(consumerSource, /formatSvgImportStylePresentation\(/u)
  assert.match(consumerSource, /formatSvgImportWarningPresentation\(/u)
  assert.doesNotMatch(consumerSource, /\blocale\s*(?:===|!==)/u)
  assert.doesNotMatch(
    consumerSource,
    /formatLocalizedText|selectLocalizedText/u,
  )
  assert.doesNotMatch(consumerSource, /[ぁ-んァ-ン一-龯]/u)
  assert.doesNotMatch(
    consumerSource,
    /Paper boundary|Mountain fold|Some SVG information/u,
  )

  const rawReturn = presentationSource.indexOf(
    "if (locale === 'ja') return warning",
  )
  const classification = presentationSource.indexOf(
    'classifySvgImportNativeWarning(warning)',
  )
  assert.ok(rawReturn >= 0)
  assert.ok(classification > rawReturn)
  assert.doesNotMatch(presentationSource, /svgImportDialogText/u)
  assert.match(
    nativeInputSource,
    /\^表示上限により\(\[0-9\]\+\)本の線/u,
  )
  assert.match(nativeInputSource, /\/（\(\[0-9\]\+\)件）。\$\/u/u)
  assert.doesNotMatch(
    nativeInputSource,
    /Some SVG|Unsupported SVG|Paper boundary/u,
  )
})

function assertDeepFrozen(value: unknown, seen = new Set<object>()): void {
  if (!value || typeof value !== 'object' || seen.has(value)) return
  seen.add(value)
  assert.equal(Object.isFrozen(value), true)
  for (const nested of Object.values(value)) {
    assertDeepFrozen(nested, seen)
  }
}

function placeholders(value: string): string[] {
  return [...value.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\}/gu)]
    .map((match) => match[1]!)
    .sort()
}

function assertLocalizedLeaves(value: unknown): number {
  assert.ok(value && typeof value === 'object')
  const record = value as Record<string, unknown>
  const keys = Object.keys(record)
  if (keys.length === 2 && keys[0] === 'ja' && keys[1] === 'en') {
    assert.equal(typeof record.ja, 'string')
    assert.equal(typeof record.en, 'string')
    assert.notEqual(record.ja, '')
    assert.notEqual(record.en, '')
    assert.deepEqual(
      placeholders(record.ja as string),
      placeholders(record.en as string),
    )
    return 1
  }
  return Object.values(record).reduce(
    (count, nested) => count + assertLocalizedLeaves(nested),
    0,
  )
}
