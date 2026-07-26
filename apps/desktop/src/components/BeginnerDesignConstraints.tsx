import { APP_TEXT } from '../lib/appText.ts'
import type { ProjectSnapshot } from '../lib/coreClient.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
} from '../lib/i18n.ts'
import type { useBeginnerEditorState } from '../lib/useBeginnerEditorState.ts'
import { BeginnerShapeCanvasPreview } from './BeginnerShapeCanvasPreview.tsx'
import { BeginnerSkeletonEditor } from './BeginnerSkeletonEditor.tsx'
import { BeginnerProtrusionEditor } from './BeginnerProtrusionEditor.tsx'
import { GenericBodyOutlineEditor } from './GenericBodyOutlineEditor.tsx'

type EditorState = ReturnType<typeof useBeginnerEditorState>

export function BeginnerDesignConstraints({
  locale,
  snapshot,
  coreBusy,
  recoveryBlocking,
  selectedFaceId,
  editor,
}: Readonly<{
  locale: Locale
  snapshot: ProjectSnapshot
  coreBusy: boolean
  recoveryBlocking: boolean
  selectedFaceId: string | null
  editor: EditorState
}>) {
  const text = (localized: Parameters<typeof selectLocalizedText>[1]) => (
    selectLocalizedText(locale, localized)
  )
  const formattedText = (
    localized: Parameters<typeof formatLocalizedText>[1],
    variables: Parameters<typeof formatLocalizedText>[2],
  ) => formatLocalizedText(locale, localized, variables)
  const {
    beginnerPartTotal,
    setBeginnerPartTotal,
    beginnerProtrusions,
    setBeginnerProtrusions,
    beginnerBodyOutline,
    setBeginnerBodyOutline,
    beginnerBodySize,
    setBeginnerBodySize,
    beginnerBodyOutlineMode,
    setBeginnerBodyOutlineMode,
    beginnerBulgeTargets,
    setBeginnerBulgeTargets,
    addBeginnerBulgeTarget,
  } = editor
  const constraints = snapshot.beginner_design_profile.generation_constraints

  return (
    <>
      <fieldset
        aria-describedby="beginner-target-parts-help beginner-target-parts-total"
        onInput={(event) => {
          const inputs = event.currentTarget.querySelectorAll<HTMLInputElement>(
            'input[name^="target_part_"]',
          )
          setBeginnerPartTotal(Array.from(inputs).reduce(
            (sum, input) => sum + Math.max(0, Number(input.value) || 0),
            0,
          ))
        }}
      >
        <legend>{text(APP_TEXT.targetShapeParts)}</legend>
        {([
          ['head', APP_TEXT.head2],
          ['torso', APP_TEXT.torso2],
          ['leg', APP_TEXT.legs],
          ['horn', APP_TEXT.horns],
          ['ear', APP_TEXT.ears],
          ['wing', APP_TEXT.wings],
          ['tail', APP_TEXT.tails],
        ] as const).map(([kind, label]) => (
          <label className="field" key={kind}>
            <span>{text(label)}</span>
            <input
              name={`target_part_${kind}`}
              type="number"
              min={kind === 'head' || kind === 'torso' ? 1 : 0}
              max={8}
              required={kind === 'head' || kind === 'torso'}
              defaultValue={constraints.target_parts.find(
                (part) => part.kind === kind,
              )?.count ?? (kind === 'head' || kind === 'torso' ? 1 : 0)}
              disabled={coreBusy || recoveryBlocking}
            />
          </label>
        ))}
      </fieldset>
      <fieldset aria-describedby="beginner-body-size-help">
        <legend>{text(APP_TEXT.targetBodySizeOptional)}</legend>
        <label className="field">
          <span>{text(APP_TEXT.bodyWidthMm)}</span>
          <input
            name="generic_body_width_mm"
            type="number"
            min={0.1}
            max={100000}
            step={0.1}
            value={beginnerBodySize?.[0] === undefined
              ? ''
              : beginnerBodySize[0] / 10}
            onChange={(event) => {
              const value = Number(event.currentTarget.value)
              setBeginnerBodySize((current) =>
                event.currentTarget.value === ''
                  ? undefined
                  : [
                      Math.round(value * 10),
                      current?.[1] ?? Math.round(value * 10),
                    ])
            }}
          />
        </label>
        <label className="field">
          <span>{text(APP_TEXT.bodyHeightMm)}</span>
          <input
            name="generic_body_height_mm"
            type="number"
            min={0.1}
            max={100000}
            step={0.1}
            value={beginnerBodySize?.[1] === undefined
              ? ''
              : beginnerBodySize[1] / 10}
            onChange={(event) => {
              const value = Number(event.currentTarget.value)
              setBeginnerBodySize((current) =>
                event.currentTarget.value === ''
                  ? undefined
                  : [
                      current?.[0] ?? Math.round(value * 10),
                      Math.round(value * 10),
                    ])
            }}
          />
        </label>
        <p id="beginner-body-size-help" className="muted">
          {text(APP_TEXT.leaveBothFieldsBlankForNoBodySizeTargetA)}
        </p>
      </fieldset>
      <GenericBodyOutlineEditor
        locale={locale}
        points={beginnerBodyOutline}
        mode={beginnerBodyOutlineMode}
        onModeChange={(mode) => {
          setBeginnerBodyOutlineMode(mode)
          setBeginnerBodyOutline([])
        }}
        onChange={setBeginnerBodyOutline}
      />
      <BeginnerShapeCanvasPreview
        locale={locale}
        bodySize={beginnerBodySize}
        bodyOutline={beginnerBodyOutline}
        bodyMode={beginnerBodyOutlineMode}
        protrusions={beginnerProtrusions}
        onBodyOutlineChange={setBeginnerBodyOutline}
        onProtrusionChange={(changed) =>
          setBeginnerProtrusions((targets) => targets.map(
            (target) => target.id === changed.id ? changed : target,
          ))}
      />
      <output id="beginner-target-parts-total" aria-live="polite">
        {formattedText(APP_TEXT.totalPartsTotal32, {
          total: beginnerPartTotal,
        })}
      </output>
      <p id="beginner-target-parts-help" className="muted">
        {text(APP_TEXT.oneHeadAndOneTorsoAreRequiredEachPartIs)}
      </p>
      <BeginnerSkeletonEditor
        locale={locale}
        coreBusy={coreBusy}
        recoveryBlocking={recoveryBlocking}
        editor={editor}
      />
      <BeginnerProtrusionEditor
        locale={locale}
        coreBusy={coreBusy}
        editor={editor}
      />
      <fieldset aria-describedby="beginner-bulge-help">
        <legend>{text(APP_TEXT.text3dBulgeTargets)}</legend>
        <p>
          {selectedFaceId
            ? formattedText(APP_TEXT.selectedFaceId, { id: selectedFaceId })
            : text(APP_TEXT.selectATargetFaceInThe2DOr3DView)}
        </p>
        {([
          ['bulge_min_x', 'Range minimum X (mm)', -5],
          ['bulge_min_y', 'Range minimum Y (mm)', -5],
          ['bulge_min_z', 'Range minimum Z (mm)', -5],
          ['bulge_max_x', 'Range maximum X (mm)', 5],
          ['bulge_max_y', 'Range maximum Y (mm)', 5],
          ['bulge_max_z', 'Range maximum Z (mm)', 5],
          ['bulge_direction_x', 'Bulge direction X', 0],
          ['bulge_direction_y', 'Bulge direction Y', 0],
          ['bulge_direction_z', 'Bulge direction Z', 1],
          ['bulge_amount_mm', 'Bulge amount (mm)', 5],
        ] as const).map(([name, label, initial]) => (
          <label className="field" key={name}>
            <span>{label}</span>
            <input
              name={name}
              type="number"
              step={name.includes('direction') ? 0.001 : 0.1}
              min={name === 'bulge_amount_mm'
                ? 0.1
                : name.includes('direction') ? -1 : -10000}
              max={name === 'bulge_amount_mm'
                ? 100000
                : name.includes('direction') ? 1 : 10000}
              defaultValue={initial}
              required
            />
          </label>
        ))}
        <button
          type="button"
          disabled={
            !selectedFaceId || beginnerBulgeTargets.length >= 32 || coreBusy
          }
          onClick={(event) => event.currentTarget.form
            && addBeginnerBulgeTarget(event.currentTarget.form)}
        >
          {text(APP_TEXT.addBulgeTargetForSelectedFace)}
        </button>
        <ul aria-label={text(APP_TEXT.text3dBulgeTargetList)}>
          {beginnerBulgeTargets.map((target) => (
            <li key={target.id}>
              {formattedText(APP_TEXT.faceFaceAmountAmountMm, {
                face: target.face_ids[0],
                amount: target.amount_tenths_mm / 10,
              })}
              <button
                type="button"
                onClick={() => setBeginnerBulgeTargets(
                  (targets) => targets.filter(
                    (item) => item.id !== target.id,
                  ),
                )}
              >
                {text(APP_TEXT.remove)}
              </button>
            </li>
          ))}
        </ul>
      </fieldset>
      <p id="beginner-bulge-help" className="muted">
        {text(APP_TEXT.storesOnlyTheBoundedRangeDirectionAndAmountBoundTo)}
      </p>
      <label className="field">
        <span>{text(APP_TEXT.maximumSteps)}</span>
        <input
          name="maximum_steps"
          type="number"
          min={1}
          max={500}
          required
          defaultValue={constraints.maximum_steps}
          disabled={coreBusy || recoveryBlocking}
        />
      </label>
      <label className="field">
        <span>{text(APP_TEXT.partDetail)}</span>
        <select
          name="detail_level"
          defaultValue={constraints.detail_level}
          disabled={coreBusy || recoveryBlocking}
        >
          <option value="simple">{text(APP_TEXT.simple)}</option>
          <option value="standard">{text(APP_TEXT.standard)}</option>
          <option value="detailed">{text(APP_TEXT.detailed)}</option>
        </select>
      </label>
      <label className="field">
        <span>{text(APP_TEXT.allowedFoldTechniques)}</span>
        <select
          name="allowed_techniques"
          multiple
          size={8}
          required
          defaultValue={constraints.allowed_techniques}
          disabled={coreBusy || recoveryBlocking}
          aria-describedby="beginner-technique-help"
        >
          <option value="valley_fold">{text(APP_TEXT.valleyFold)}</option>
          <option value="mountain_fold">{text(APP_TEXT.mountainFold)}</option>
          <option value="inside_reverse_fold">
            {text(APP_TEXT.insideReverseFold)}
          </option>
          <option value="outside_reverse_fold">
            {text(APP_TEXT.outsideReverseFold)}
          </option>
          <option value="squash_fold">{text(APP_TEXT.squashFold)}</option>
          <option value="petal_fold">{text(APP_TEXT.petalFold)}</option>
          <option value="sink_fold">{text(APP_TEXT.sinkFold)}</option>
          <option value="crimp_fold">{text(APP_TEXT.crimpFold)}</option>
        </select>
      </label>
      <p id="beginner-technique-help" className="muted">
        {text(APP_TEXT.holdCtrlOrCommandToSelectMultipleTechniquesSelectAt)}
      </p>
      <p className="muted" data-testid="petal-fold-certification-scope">
        {text(APP_TEXT.petalFoldIsADesignPreferenceOnlyItsPhysicalMotion)}
      </p>
      <button type="submit" disabled={coreBusy || recoveryBlocking}>
        {text(APP_TEXT.saveDesignPriorities)}
      </button>
    </>
  )
}
