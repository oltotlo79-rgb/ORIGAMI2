import type {
  ComponentProps,
  FormEventHandler,
} from 'react'

import { APP_TEXT } from '../lib/appText.ts'
import { rgbaToHex } from '../lib/appElementMetadata.ts'
import type { ProjectSnapshot } from '../lib/coreClient.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
} from '../lib/i18n.ts'
import {
  formatLengthInput,
  type BoundaryLengthReference,
  type ResolvedLengthDisplayUnit,
} from '../lib/lengthUnit.ts'
import {
  builtinPaperPatternFromAsset,
} from '../lib/paperPatterns.ts'
import { formatPaperThicknessInput } from '../lib/paperThicknessInput.ts'
import { MAX_NUMERIC_EXPRESSION_SOURCE_BYTES } from '../lib/numericExpressionNative.ts'
import { CreationDimensionExpressionSummary } from './CreationDimensionExpressionSummary.tsx'
import { LengthUnitControl } from './LengthUnitControl.tsx'
import { PaperThicknessInput } from './PaperThicknessInput.tsx'

type PaperSize = Readonly<{
  width: number
  height: number
}>

export type PaperInspectorSectionProps = Readonly<{
  locale: Locale
  snapshot: ProjectSnapshot | null
  coreBusy: boolean
  lengthDisplayUnit: ResolvedLengthDisplayUnit
  lengthDisplayUnitLabelText: string
  boundaryLengthReferences: readonly BoundaryLengthReference[]
  paperFormKey: string
  paperResizeFormKey: string
  rectangularPaperSize: PaperSize | null
  rectangularRatioReferenceAxis: 'width' | 'height' | null
  creationDimensionExpression:
    ComponentProps<typeof CreationDimensionExpressionSummary>['binding']
  onLengthUnitChange: ComponentProps<typeof LengthUnitControl>['onChange']
  onSubmitPaperProperties: FormEventHandler<HTMLFormElement>
  onChooseFrontPaperTexture: () => void
  onChooseBackPaperTexture: () => void
  onSubmitPaperResize: FormEventHandler<HTMLFormElement>
}>

export function PaperInspectorSection({
  locale,
  snapshot,
  coreBusy,
  lengthDisplayUnit,
  lengthDisplayUnitLabelText,
  boundaryLengthReferences,
  paperFormKey,
  paperResizeFormKey,
  rectangularPaperSize,
  rectangularRatioReferenceAxis,
  creationDimensionExpression,
  onLengthUnitChange,
  onSubmitPaperProperties,
  onChooseFrontPaperTexture,
  onChooseBackPaperTexture,
  onSubmitPaperResize,
}: PaperInspectorSectionProps) {
  const text = (localized: Parameters<typeof selectLocalizedText>[1]) => (
    selectLocalizedText(locale, localized)
  )
  const formattedText = (
    localized: Parameters<typeof formatLocalizedText>[1],
    variables: Parameters<typeof formatLocalizedText>[2],
  ) => formatLocalizedText(locale, localized, variables)

  return (
    <section>
      <h2>{text(APP_TEXT.paper)}</h2>
      <LengthUnitControl
        unit={lengthDisplayUnit}
        references={boundaryLengthReferences}
        disabled={coreBusy || !snapshot}
        onChange={onLengthUnitChange}
      />
      <form
        key={paperFormKey}
        className="paper-properties-form"
        onSubmit={onSubmitPaperProperties}
        noValidate
      >
        <div className="field">
          <label htmlFor="paper-thickness-mm">
            {text(APP_TEXT.thickness2)}
          </label>
          <PaperThicknessInput
            id="paper-thickness-mm"
            name="thickness_display"
            initialValue={lengthDisplayUnit.effectiveUnit === 'mm'
              ? formatPaperThicknessInput(snapshot?.paper.thickness_mm)
              : formatLengthInput(
                  snapshot?.paper.thickness_mm,
                  lengthDisplayUnit,
                )}
            sourceMillimetres={snapshot?.paper.thickness_mm}
            unit={lengthDisplayUnit}
            disabled={coreBusy || !snapshot}
          />
          <span>{lengthDisplayUnitLabelText}</span>
        </div>
        <div className="paper-color-fields">
          <label className="paper-color-field">
            <span>{text(APP_TEXT.frontColor)}</span>
            <input
              name="front_color"
              type="color"
              defaultValue={rgbaToHex(snapshot?.paper.front.color, '#ffffff')}
              disabled={coreBusy || !snapshot}
            />
          </label>
          <label className="paper-color-field">
            <span>{text(APP_TEXT.backColor)}</span>
            <input
              name="back_color"
              type="color"
              defaultValue={rgbaToHex(snapshot?.paper.back.color, '#f8f8f5')}
              disabled={coreBusy || !snapshot}
            />
          </label>
        </div>
        <div className="paper-color-fields">
          <label className="paper-color-field">
            <span>{text(APP_TEXT.frontPattern)}</span>
            <select
              name="front_pattern"
              defaultValue={builtinPaperPatternFromAsset(
                snapshot?.paper.front.texture_asset,
              ) ?? (snapshot?.paper.front.texture_asset ? 'custom' : 'none')}
              disabled={coreBusy || !snapshot}
            >
              <option value="none">{text(APP_TEXT.noneSolid)}</option>
              <option value="dots">{text(APP_TEXT.dots)}</option>
              <option value="grid">{text(APP_TEXT.grid2)}</option>
              <option value="stripes">{text(APP_TEXT.stripes)}</option>
              {snapshot?.paper.front.texture_asset
                && !builtinPaperPatternFromAsset(
                  snapshot.paper.front.texture_asset,
                )
                ? (
                    <option value="custom">
                      {text(APP_TEXT.importedImage)}
                    </option>
                  )
                : null}
            </select>
            <button
              type="button"
              disabled={coreBusy || !snapshot}
              onClick={onChooseFrontPaperTexture}
            >
              {text(APP_TEXT.importImage)}
            </button>
          </label>
          <label className="paper-color-field">
            <span>{text(APP_TEXT.backPattern)}</span>
            <select
              name="back_pattern"
              defaultValue={builtinPaperPatternFromAsset(
                snapshot?.paper.back.texture_asset,
              ) ?? (snapshot?.paper.back.texture_asset ? 'custom' : 'none')}
              disabled={coreBusy || !snapshot}
            >
              <option value="none">{text(APP_TEXT.noneSolid)}</option>
              <option value="dots">{text(APP_TEXT.dots)}</option>
              <option value="grid">{text(APP_TEXT.grid2)}</option>
              <option value="stripes">{text(APP_TEXT.stripes)}</option>
              {snapshot?.paper.back.texture_asset
                && !builtinPaperPatternFromAsset(
                  snapshot.paper.back.texture_asset,
                )
                ? (
                    <option value="custom">
                      {text(APP_TEXT.importedImage)}
                    </option>
                  )
                : null}
            </select>
            <button
              type="button"
              disabled={coreBusy || !snapshot}
              onClick={onChooseBackPaperTexture}
            >
              {text(APP_TEXT.importImage)}
            </button>
          </label>
        </div>
        <label className="check">
          <input
            name="cutting_allowed"
            type="checkbox"
            defaultChecked={snapshot?.paper.cutting_allowed ?? false}
            disabled={coreBusy || !snapshot}
          />{' '}
          {text(APP_TEXT.allowCutting)}
        </label>
        <div className="property-actions">
          <button type="submit" disabled={coreBusy || !snapshot}>
            {text(APP_TEXT.updatePaperSettings)}
          </button>
        </div>
      </form>
      <div className="paper-size-editor">
        <h3>{text(APP_TEXT.paperSize)}</h3>
        <form
          key={paperResizeFormKey}
          className="paper-size-form"
          onSubmit={onSubmitPaperResize}
          noValidate
        >
          <div className="paper-size-fields">
            <label className="field">
              <span>{text(APP_TEXT.width)}</span>
              <input
                name="width_display"
                type="text"
                inputMode="text"
                maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
                defaultValue={formatLengthInput(
                  rectangularPaperSize?.width ?? 0,
                  lengthDisplayUnit,
                )}
                readOnly={rectangularRatioReferenceAxis === 'width'}
                required
                disabled={coreBusy || !rectangularPaperSize}
                aria-label={formattedText(APP_TEXT.paperWidthUnit, {
                  unit: lengthDisplayUnitLabelText,
                })}
              />
              <span>{lengthDisplayUnitLabelText}</span>
            </label>
            <label className="field">
              <span>{text(APP_TEXT.height)}</span>
              <input
                name="height_display"
                type="text"
                inputMode="text"
                maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
                defaultValue={formatLengthInput(
                  rectangularPaperSize?.height ?? 0,
                  lengthDisplayUnit,
                )}
                readOnly={rectangularRatioReferenceAxis === 'height'}
                required
                disabled={coreBusy || !rectangularPaperSize}
                aria-label={formattedText(APP_TEXT.paperHeightUnit, {
                  unit: lengthDisplayUnitLabelText,
                })}
              />
              <span>{lengthDisplayUnitLabelText}</span>
            </label>
          </div>
          {!rectangularPaperSize && (
            <p className="paper-size-note">
              {text(APP_TEXT.paperThatIsNotRecognizedAsAnAxisAlignedRectangle)}
            </p>
          )}
          <p className="paper-size-note">
            {text(APP_TEXT.resizingProportionallyTransformsEveryVertexIncludingFoldLinesFromThe)}
          </p>
          <CreationDimensionExpressionSummary
            key={snapshot?.project_id ?? 'no-project'}
            binding={creationDimensionExpression}
          />
          {rectangularRatioReferenceAxis && (
            <p className="paper-size-note">
              {formattedText(
                APP_TEXT.forAPaperEdgeRatioAxisRemainsReadOnlyAt,
                {
                  axis: rectangularRatioReferenceAxis === 'width'
                    ? text(APP_TEXT.width2)
                    : text(APP_TEXT.height2),
                },
              )}
            </p>
          )}
          <div className="property-actions">
            <button
              type="submit"
              disabled={coreBusy || !snapshot || !rectangularPaperSize}
            >
              {text(APP_TEXT.resizePaper)}
            </button>
          </div>
        </form>
      </div>
    </section>
  )
}
