import type { FormEventHandler } from 'react'

import { lineKindLabel } from '../lib/appPresentation.ts'
import { APP_TEXT } from '../lib/appText.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
} from '../lib/i18n.ts'
import { MAX_NUMERIC_EXPRESSION_SOURCE_BYTES } from '../lib/numericExpressionNative.ts'
import type {
  CreaseCanvasFace,
  CreaseLine,
} from './CreaseCanvas.tsx'

export type SelectedFaceInspectorProps = Readonly<{
  locale: Locale
  face: CreaseCanvasFace
  removableEdges: readonly CreaseLine[]
  locked: boolean
  coreBusy: boolean
  cuttingAllowed: boolean
  displayUnitLabel: string
  onSubmitMove: FormEventHandler<HTMLFormElement>
  onSubmitSplit: FormEventHandler<HTMLFormElement>
  onSubmitMerge: FormEventHandler<HTMLFormElement>
}>

export function SelectedFaceInspector({
  locale,
  face,
  removableEdges,
  locked,
  coreBusy,
  cuttingAllowed,
  displayUnitLabel,
  onSubmitMove,
  onSubmitSplit,
  onSubmitMerge,
}: SelectedFaceInspectorProps) {
  const text = (localized: Parameters<typeof selectLocalizedText>[1]) => (
    selectLocalizedText(locale, localized)
  )
  const formattedText = (
    localized: Parameters<typeof formatLocalizedText>[1],
    variables: Parameters<typeof formatLocalizedText>[2],
  ) => formatLocalizedText(locale, localized, variables)

  return (
    <>
      <dl>
        <div><dt>ID</dt><dd>{face.id}</dd></div>
        <div>
          <dt>{text(APP_TEXT.boundaryVertices)}</dt>
          <dd>{face.vertexIds.length}</dd>
        </div>
        <div>
          <dt>{text(APP_TEXT.boundaryLines)}</dt>
          <dd>{face.edgeIds.length}</dd>
        </div>
      </dl>
      <form onSubmit={onSubmitMove}>
        <fieldset disabled={coreBusy || locked}>
          <legend>{text(APP_TEXT.moveEntireFace)}</legend>
          <label className="field">
            {formattedText(APP_TEXT.horizontalOffsetUnit, {
              unit: displayUnitLabel,
            })}
            <input
              name="face_delta_x_display"
              type="text"
              inputMode="text"
              maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
              defaultValue="0"
            />
          </label>
          <label className="field">
            {formattedText(APP_TEXT.verticalOffsetUnit, {
              unit: displayUnitLabel,
            })}
            <input
              name="face_delta_y_display"
              type="text"
              inputMode="text"
              maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
              defaultValue="0"
            />
          </label>
          <div className="property-actions">
            <button type="submit">{text(APP_TEXT.moveEntireFace)}</button>
          </div>
        </fieldset>
      </form>
      <form onSubmit={onSubmitSplit}>
        <fieldset disabled={coreBusy || locked || face.vertexIds.length < 4}>
          <legend>{text(APP_TEXT.addOrSplitAFace)}</legend>
          <label className="field">
            {text(APP_TEXT.startVertex)}
            <select
              name="face_split_start"
              defaultValue={face.vertexIds[0]}
            >
              {face.vertexIds.map((vertexId, index) => (
                <option value={vertexId} key={vertexId}>
                  {formattedText(APP_TEXT.vertexIndexId, {
                    index: index + 1,
                    id: vertexId,
                  })}
                </option>
              ))}
            </select>
          </label>
          <label className="field">
            {text(APP_TEXT.endVertex)}
            <select
              name="face_split_end"
              defaultValue={face.vertexIds[2]}
            >
              {face.vertexIds.map((vertexId, index) => (
                <option value={vertexId} key={vertexId}>
                  {formattedText(APP_TEXT.vertexIndexId, {
                    index: index + 1,
                    id: vertexId,
                  })}
                </option>
              ))}
            </select>
          </label>
          <label className="field">
            {text(APP_TEXT.splitLineType)}
            <select name="face_split_kind" defaultValue="mountain">
              <option value="mountain">{text(APP_TEXT.mountainFold)}</option>
              <option value="valley">{text(APP_TEXT.valleyFold)}</option>
              <option value="auxiliary">
                {text(APP_TEXT.auxiliaryLine)}
              </option>
              {cuttingAllowed && (
                <option value="cut">{text(APP_TEXT.cut2)}</option>
              )}
            </select>
          </label>
          <div className="property-actions">
            <button type="submit">{text(APP_TEXT.splitAndAddFace)}</button>
          </div>
        </fieldset>
      </form>
      <form onSubmit={onSubmitMerge}>
        <fieldset disabled={coreBusy || locked || removableEdges.length === 0}>
          <legend>{text(APP_TEXT.deleteOrMergeFace)}</legend>
          <label className="field">
            {text(APP_TEXT.sharedLineToRemove)}
            <select name="face_merge_edge">
              {removableEdges.map((line) => (
                <option value={line.id} key={line.id}>
                  {lineKindLabel(line.kind, locale)}: {line.id}
                </option>
              ))}
            </select>
          </label>
          <div className="property-actions">
            <button type="submit" className="danger">
              {text(APP_TEXT.removeLineAndMergeFace)}
            </button>
          </div>
        </fieldset>
      </form>
      {locked && (
        <p className="muted">
          {text(APP_TEXT.thisFaceCannotMoveBecauseItsBoundaryIncludesALocked)}
        </p>
      )}
    </>
  )
}
