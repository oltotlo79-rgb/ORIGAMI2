import { APP_TEXT } from '../lib/appText.ts'
import {
  selectLocalizedText,
  type Locale,
} from '../lib/i18n.ts'
import type { useBeginnerEditorState } from '../lib/useBeginnerEditorState.ts'
import { ProtrusionDimensionEditor } from './ProtrusionDimensionEditor.tsx'

type EditorState = ReturnType<typeof useBeginnerEditorState>

export function BeginnerProtrusionEditor({
  locale,
  coreBusy,
  editor,
}: Readonly<{
  locale: Locale
  coreBusy: boolean
  editor: EditorState
}>) {
  const text = (localized: Parameters<typeof selectLocalizedText>[1]) => (
    selectLocalizedText(locale, localized)
  )
  const {
    beginnerProtrusions,
    setBeginnerProtrusions,
    beginnerProtrusionKinds,
    setBeginnerProtrusionKinds,
    addBeginnerProtrusion,
    createEmptyGenericTarget,
  } = editor

  return (
    <>
      <fieldset aria-describedby="beginner-protrusion-help">
        <legend>{text(APP_TEXT.protrusionTargets)}</legend>
        {([
          ['protrusion_count', APP_TEXT.count, 2, 1, 8, 1],
          ['protrusion_length_mm', APP_TEXT.lengthMm, 20, 0.1, 100000, 0.1],
          ['protrusion_thickness_mm', APP_TEXT.thicknessMm, 2, 0.1, 1000, 0.1],
          ['protrusion_position_x_mm', APP_TEXT.finalPositionXMm, 0, -10000, 10000, 0.1],
          ['protrusion_position_y_mm', APP_TEXT.finalPositionYMm, 0, -10000, 10000, 0.1],
          ['protrusion_position_z_mm', APP_TEXT.finalPositionZMm, 0, -10000, 10000, 0.1],
          ['protrusion_direction_x', APP_TEXT.directionX, 1, -1, 1, 0.001],
          ['protrusion_direction_y', APP_TEXT.directionY, 0, -1, 1, 0.001],
          ['protrusion_direction_z', APP_TEXT.directionZ, 0, -1, 1, 0.001],
          ['protrusion_curvature_degrees', APP_TEXT.curvatureDegrees, 0, -360, 360, 1],
          ['protrusion_motion_min', APP_TEXT.motionMinimumDegrees, 0, -360, 360, 1],
          ['protrusion_motion_max', APP_TEXT.motionMaximumDegrees, 0, -360, 360, 1],
          ['protrusion_priority', APP_TEXT.priority, 50, 1, 100, 1],
        ] as const).map(([name, label, initial, min, max, step]) => (
          <label className="field" key={name}>
            <span>{text(label)}</span>
            <input
              name={name}
              type="number"
              defaultValue={initial}
              min={min}
              max={max}
              step={step}
              required
            />
          </label>
        ))}
        {([
          ['protrusion_root_width_mm', APP_TEXT.rootWidthMmOptional],
          ['protrusion_tip_width_mm', APP_TEXT.tipWidthMmOptional],
        ] as const).map(([name, label]) => (
          <label className="field" key={name}>
            <span>{text(label)}</span>
            <input
              name={name}
              type="number"
              min={0.1}
              max={1000}
              step={0.1}
            />
          </label>
        ))}
        <label className="field">
          <span>{text(APP_TEXT.symmetry)}</span>
          <select name="protrusion_symmetry" defaultValue="bilateral">
            <option value="none">{text(APP_TEXT.none)}</option>
            <option value="bilateral">{text(APP_TEXT.bilateral)}</option>
            <option value="radial">{text(APP_TEXT.radial)}</option>
          </select>
        </label>
        <label className="field">
          <span>{text(APP_TEXT.joint)}</span>
          <select name="protrusion_joint" defaultValue="fixed">
            <option value="fixed">{text(APP_TEXT.fixed)}</option>
            <option value="hinge">{text(APP_TEXT.hinge)}</option>
            <option value="ball">{text(APP_TEXT.ball)}</option>
          </select>
        </label>
        <label className="field">
          <span>{text(APP_TEXT.side)}</span>
          <select name="protrusion_side" defaultValue="either">
            <option value="front">{text(APP_TEXT.front)}</option>
            <option value="back">{text(APP_TEXT.back)}</option>
            <option value="either">{text(APP_TEXT.either)}</option>
          </select>
        </label>
        <button
          type="button"
          disabled={beginnerProtrusions.length >= 32 || coreBusy}
          onClick={(event) => event.currentTarget.form
            && addBeginnerProtrusion(event.currentTarget.form)}
        >
          {text(APP_TEXT.addProtrusionTarget)}
        </button>
        {beginnerProtrusions.length === 0 && (
          <button
            type="button"
            disabled={coreBusy}
            onClick={createEmptyGenericTarget}
          >
            {text(APP_TEXT.createEmptyGenericTarget)}
          </button>
        )}
        {beginnerProtrusions.length > 0 && (
          <table aria-label={text(APP_TEXT.featureConstraintComparison)}>
            <thead>
              <tr>
                <th>{text(APP_TEXT.feature)}</th>
                <th>{text(APP_TEXT.length)}</th>
                <th>{text(APP_TEXT.thickness)}</th>
                <th>{text(APP_TEXT.joint)}</th>
                <th>{text(APP_TEXT.motion)}</th>
                <th>{text(APP_TEXT.side2)}</th>
                <th>{text(APP_TEXT.priority)}</th>
              </tr>
            </thead>
            <tbody>
              {beginnerProtrusions.map((target, index) => (
                <tr key={target.id}>
                  <td>
                    {beginnerProtrusionKinds[index] ?? 'tail'} #{target.id}
                  </td>
                  <td>{target.length_tenths_mm / 10} mm</td>
                  <td>{target.thickness_tenths_mm / 10} mm</td>
                  <td>{target.joint}</td>
                  <td>{target.motion_degrees.join('..')}°</td>
                  <td>{target.side}</td>
                  <td>{target.priority}/100</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        <ul aria-label={text(APP_TEXT.protrusionTargetList)}>
          {beginnerProtrusions.map((target, index) => (
            <ProtrusionDimensionEditor
              key={target.id}
              locale={locale}
              target={target}
              kind={beginnerProtrusionKinds[index] ?? 'tail'}
              onKindChange={(kind) =>
                setBeginnerProtrusionKinds((kinds) =>
                  kinds.length === beginnerProtrusions.length
                    ? kinds.map((item, kindIndex) =>
                        kindIndex === index ? kind : item)
                    : beginnerProtrusions.map((_, kindIndex) =>
                        kindIndex === index ? kind : 'tail'))}
              onChange={(changed) =>
                setBeginnerProtrusions((targets) => targets.map(
                  (item) => item.id === changed.id ? changed : item,
                ))}
              onRemove={() => {
                setBeginnerProtrusions((targets) => targets
                  .filter((item) => item.id !== target.id)
                  .map((item, canonicalIndex) => ({
                    ...item,
                    id: canonicalIndex + 1,
                  })))
                setBeginnerProtrusionKinds((kinds) =>
                  kinds.filter((_, kindIndex) => kindIndex !== index))
              }}
              canRemove={beginnerProtrusions.length !== 2}
              canMoveUp={index > 0}
              canMoveDown={index + 1 < beginnerProtrusions.length}
              onMoveUp={() => {
                setBeginnerProtrusions((targets) => {
                  if (index === 0) return targets
                  const moved = [...targets]
                  ;[moved[index - 1], moved[index]] = [
                    moved[index]!,
                    moved[index - 1]!,
                  ]
                  return moved.map((item, canonicalIndex) => ({
                    ...item,
                    id: canonicalIndex + 1,
                  }))
                })
                setBeginnerProtrusionKinds((kinds) => {
                  if (index === 0) return kinds
                  const moved = [...kinds]
                  ;[moved[index - 1], moved[index]] = [
                    moved[index]!,
                    moved[index - 1]!,
                  ]
                  return moved
                })
              }}
              onMoveDown={() => {
                setBeginnerProtrusions((targets) => {
                  if (index + 1 >= targets.length) return targets
                  const moved = [...targets]
                  ;[moved[index], moved[index + 1]] = [
                    moved[index + 1]!,
                    moved[index]!,
                  ]
                  return moved.map((item, canonicalIndex) => ({
                    ...item,
                    id: canonicalIndex + 1,
                  }))
                })
                setBeginnerProtrusionKinds((kinds) => {
                  if (index + 1 >= kinds.length) return kinds
                  const moved = [...kinds]
                  ;[moved[index], moved[index + 1]] = [
                    moved[index + 1]!,
                    moved[index]!,
                  ]
                  return moved
                })
              }}
            />
          ))}
        </ul>
      </fieldset>
      <p id="beginner-protrusion-help" className="muted">
        {text(APP_TEXT.explicitlySetsCountDimensionsFinalPositionDirectionSymmetryCurvatureJoin)}
      </p>
    </>
  )
}
