import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const appSource = readSource('../src/App.tsx')
const clientSource = readSource('../src/lib/coreClient.ts')
const dialogSource = readSource('../src/components/SvgImportDialog.tsx')
const svgImportSource = readSource('../src/lib/svgImport.ts')
const nativeSource = readSource('../src-tauri/src/lib.rs')
const appMessagesSource = readSource('../src/lib/appMessages.ts')

test('the client exposes only token-based preview, validation, apply, and cancel SVG invocations', () => {
  const preview = exportedFunction('previewSvgImport')
  const validate = exportedFunction('validateSvgImportSettings')
  const apply = exportedFunction('applySvgImport')
  const cancel = exportedFunction('cancelSvgImport')

  assert.match(
    preview,
    /invoke<SvgImportPreviewResponse>\('preview_svg_import'\)/u,
  )
  assert.match(
    validate,
    /invoke<SvgImportSettingsValidation>\('validate_svg_import_settings',\s*\{[\s\S]*\}\)/u,
  )
  assert.match(
    apply,
    /invoke<ProjectSnapshot>\('apply_svg_import',\s*\{[\s\S]*\}\)/u,
  )
  assert.match(
    cancel,
    /invoke<void>\('cancel_svg_import',\s*\{\s*previewId\s*\}\)/u,
  )

  for (const argument of [
    'previewId: settings.importId',
    'expectedProjectId',
    'expectedRevision',
    'name: settings.name',
    'millimetersPerUnit: settings.mmPerUnit',
    'boundaryCandidateId: settings.boundaryCandidateId',
    'validationId: settings.validationId',
    'boundaryConfirmed: settings.boundaryConfirmed',
    'styleMappings',
    'warningsAcknowledged: settings.warningsAcknowledged',
    'cuttingAllowedConfirmed: settings.cuttingAllowedConfirmed',
    'replaceDirtyProjectConfirmed',
  ]) {
    assert.match(apply, new RegExp(escapeRegExp(argument), 'u'), argument)
  }

  const clientContract = [preview, validate, apply, cancel].join('\n')
  assert.doesNotMatch(
    clientContract,
    /\b(?:path|filePath|file_path|rawSvg|raw_svg|rawXml|raw_xml)\b/iu,
    'the renderer must not receive or send a filesystem path or raw SVG/XML',
  )
  assert.equal(
    clientContract.match(
      /invoke(?:<[^>]+>)?\('(?:preview_svg_import|validate_svg_import_settings|apply_svg_import|cancel_svg_import)'/gu,
    )?.length,
    4,
  )
  for (const command of [
    'preview_svg_import',
    'validate_svg_import_settings',
    'apply_svg_import',
    'cancel_svg_import',
  ]) {
    assert.match(
      nativeSource,
      new RegExp(`#\\[tauri::command\\][\\s\\S]{0,100}(?:async )?fn ${command}\\(`, 'u'),
      `${command} native command`,
    )
    assert.match(
      nativeSource,
      new RegExp(`tauri::generate_handler!\\[[\\s\\S]*?\\b${command}\\b`, 'u'),
      `${command} command registration`,
    )
  }
  assert.match(nativeSource, /\.manage\(SvgImportState::default\(\)\)/u)
  assert.match(
    nativeSource,
    /svg_import_requires_warning_acknowledgement\(&preview\)\s*&&\s*!warnings_acknowledged/u,
  )
  assert.match(nativeSource, /if !boundary_confirmed/u)
  assert.match(
    nativeSource,
    /validate_import_scale\(millimeters_per_unit\)\?/u,
  )

  const settingsContract = sourceSection(
    svgImportSource,
    'export type SvgImportSettings',
    'export const SVG_IMPORT_TARGET_OPTIONS',
  )
  assert.match(settingsContract, /importId: string/u)
  assert.match(settingsContract, /validationId: string/u)
  assert.doesNotMatch(
    settingsContract,
    /\b(?:path|filePath|file_path|rawSvg|raw_svg|rawXml|raw_xml)\b/iu,
  )
})

test('the toolbar starts native preview without an early dirty confirmation', () => {
  const begin = appFunction('beginSvgImport', 'closeSvgImportDialog')

  assert.match(
    appSource,
    /ref=\{svgImportButtonRef\}[\s\S]*?disabled=\{coreBusy \|\| benchmarkLoading \|\| Boolean\(benchmarkRun\) \|\| !nativeSnapshot\}[\s\S]*?onClick=\{\(\) => void beginSvgImport\(\)\}[\s\S]*?aria-haspopup="dialog"[\s\S]*?SVG取込/u,
  )
  assert.match(begin, /if \(!latestSnapshotRef\.current \|\| coreOperationRef\.current\) return/u)
  assert.match(begin, /const response = await previewSvgImport\(\)/u)
  assert.match(begin, /if \(response\.canceled\)[\s\S]*?return/u)
  assert.match(begin, /if \(!response\.preview\)[\s\S]*?throw new Error/u)
  assert.match(begin, /setSvgImportPreview\(response\.preview\)/u)
  assert.doesNotMatch(
    begin,
    /\bis_dirty\b|window\.confirm|\b(?:path|filePath|file_path|rawSvg|raw_svg|rawXml|raw_xml)\b/iu,
  )

  assert.match(
    appSource,
    /\{svgImportPreview && \(\s*<SvgImportDialog[\s\S]*?preview=\{svgImportPreview\}[\s\S]*?onCancel=\{\(\) => void closeSvgImportDialog\(\)\}[\s\S]*?onImport=\{\(settings\) => void confirmSvgImport\(settings\)\}/u,
  )
})

test('dirty-project confirmation is deferred until immediately before SVG replacement', () => {
  const confirm = appFunction('confirmSvgImport', 'toggleBenchmark')
  const dirtyIndex = confirm.indexOf('current.is_dirty')
  const confirmationIndex = confirm.indexOf('window.confirm')
  const operationLeaseIndex = confirm.indexOf('coreOperationRef.current = true')
  const applyIndex = confirm.indexOf('await applySvgImport(')

  assert.ok(dirtyIndex >= 0, 'missing dirty-project guard')
  assert.ok(confirmationIndex > dirtyIndex, 'confirmation must be part of the dirty guard')
  assert.ok(
    operationLeaseIndex > confirmationIndex,
    'the import operation must not start before confirmation',
  )
  assert.ok(applyIndex > operationLeaseIndex, 'replacement must happen after confirmation')
  assert.match(
    confirm,
    /const replaceDirtyProjectConfirmed = current\.is_dirty[\s\S]*?replaceDirtyProjectConfirmed\s*&&\s*!window\.confirm\(appConfirmationText\(locale, 'replaceWithSvg'\)\)\s*\)\s*return/u,
  )
  assert.match(
    appMessagesSource,
    /replaceWithSvg:\s*\{\s*ja: '未保存の変更があります。保存せずにSVG展開図へ置き換えますか？',\s*en: 'There are unsaved changes\. Replace them with the SVG crease pattern\?'/u,
  )
  assert.match(
    confirm,
    /applySvgImport\(\s*current\.project_id,\s*current\.revision,\s*settings,\s*replaceDirtyProjectConfirmed,\s*\)/u,
  )
  assert.match(
    nativeSource,
    /project\.is_dirty\(\)\s*&&\s*!replace_dirty_project_confirmed/u,
  )
  assert.equal(
    appSource.match(/window\.confirm\(appConfirmationText\(locale, 'replaceWithSvg'\)\)/gu)?.length,
    1,
  )
})

test('the SVG modal makes every background region inert', () => {
  assert.match(
    appSource,
    /const modalOpen = newProjectOpen\s*\|\| diagnosticsDialogOpen\s*\|\| foldImportPreview !== null\s*\|\| svgImportPreview !== null/u,
  )
  assert.equal(
    appSource.match(/inert=\{modalOpen\}/gu)?.length,
    5,
    'titlebar, workspace, timeline separator, timeline, and statusbar must all be inert',
  )
  assert.match(appSource, /<header className="titlebar" inert=\{modalOpen\}>/u)
  assert.match(
    appSource,
    /<section className="workspace" inert=\{modalOpen\}[^>]*>/u,
  )
  assert.match(
    appSource,
    /<div className="workspace-timeline-separator" inert=\{modalOpen\}>/u,
  )
  assert.match(
    appSource,
    /<InstructionTimelinePanel[\s\S]*?inert=\{modalOpen\}/u,
  )
  assert.match(appSource, /<footer className="statusbar" inert=\{modalOpen\}>/u)
})

test('successful SVG apply resets editor, benchmark, and fold state only after replacement', () => {
  const confirm = appFunction('confirmSvgImport', 'toggleBenchmark')
  const tryBody = sourceSection(confirm, 'try {', '} catch (error) {')
  const catchBody = sourceSection(confirm, '} catch (error) {', '} finally {')
  const finallyBody = sourceSection(confirm, '} finally {', '\n    }\n  }')
  const applyIndex = tryBody.indexOf('await applySvgImport(')
  const snapshotIndex = tryBody.indexOf('applySnapshot(snapshot, true)')
  const closeIndex = tryBody.indexOf('setSvgImportPreview(null)')

  assert.ok(applyIndex >= 0, 'missing SVG apply call')
  assert.ok(snapshotIndex > applyIndex, 'the returned snapshot must be applied after import')
  assert.ok(closeIndex > snapshotIndex, 'the dialog must close only after snapshot application')
  for (const reset of [
    'setBenchmarkRun(null)',
    'setSelectedLineId(null)',
    'setSelectedVertexId(null)',
    'setPendingEdgeStart(null)',
    'setParallelReferenceEdgeId(null)',
    'setAppliedFoldPose(null)',
    'setFoldAngleOverrides({ projectId: null, values: new Map() })',
    'setFixedFaceChoice({ projectId: null, faceId: null })',
    "setActiveTool('select')",
  ]) {
    assert.match(tryBody, new RegExp(escapeRegExp(reset), 'u'), reset)
  }
  assert.match(
    tryBody,
    /setBenchmarkStatus\(appMessage\(\{\s*ja: 'SVG取込により通常の展開図へ戻りました',\s*en: 'Returned to the normal crease pattern after SVG import',\s*\}\)\)/u,
  )

  assert.match(
    tryBody,
    /requestAnimationFrame\(\(\) => svgImportButtonRef\.current\?\.focus\(\)\)/u,
  )
  assert.match(
    catchBody,
    /setSvgImportError\(appMessage\(\{\s*ja: '取り込めませんでした: \{error\}',\s*en: 'Could not import: \{error\}',\s*\}, \{ error: message \}\)\)/u,
  )
  assert.doesNotMatch(catchBody, /setSvgImportPreview\(null\)|setBenchmarkRun\(null\)/u)
  assert.doesNotMatch(finallyBody, /setSvgImportPreview\(null\)|setBenchmarkRun\(null\)/u)
  assert.match(finallyBody, /coreOperationRef\.current = false/u)
  assert.match(finallyBody, /setCoreBusy\(false\)/u)
})

test('cancel invalidates the SVG preview token, keeps project state, and restores focus', () => {
  const cancel = appFunction('closeSvgImportDialog', 'confirmSvgImport')
  const tryBody = sourceSection(cancel, 'try {', '} catch (error) {')
  const catchBody = sourceSection(cancel, '} catch (error) {', '} finally {')
  const finallyBody = sourceSection(cancel, '} finally {', '\n    }\n  }')

  assert.match(cancel, /const preview = svgImportPreview/u)
  assert.match(cancel, /await cancelSvgImport\(preview\.import_id\)/u)
  assert.doesNotMatch(
    cancel,
    /applySnapshot|setNativeSnapshot|setBenchmarkRun|setSelectedLineId|setSelectedVertexId/u,
  )
  assert.match(
    tryBody,
    /setSvgImportPreview\(null\)[\s\S]*?setSvgImportError\(null\)/u,
  )
  assert.match(
    tryBody,
    /requestAnimationFrame\(\(\) => svgImportButtonRef\.current\?\.focus\(\)\)/u,
  )
  assert.doesNotMatch(catchBody, /setSvgImportPreview\(null\)/u)
  assert.match(
    catchBody,
    /setSvgImportError\(appMessage\(\{\s*ja: '取消を完了できませんでした: \{error\}',\s*en: 'Could not cancel: \{error\}',\s*\}, \{ error: message \}\)\)/u,
  )
  assert.doesNotMatch(finallyBody, /setSvgImportPreview\(null\)/u)
  assert.match(finallyBody, /coreOperationRef\.current = false/u)
})

test('the SVG dialog requires native geometry validation, every mapping, and explicit confirmations', () => {
  assert.match(dialogSource, /role="dialog"/u)
  assert.match(dialogSource, /aria-modal="true"/u)
  assert.match(dialogSource, /aria-labelledby="svg-import-title"/u)
  assert.match(dialogSource, /aria-describedby="svg-import-description"/u)
  assert.match(
    dialogSource,
    /event\.key === 'Escape' && !event\.isComposing && !busy/u,
  )
  assert.match(
    dialogSource,
    /window\.addEventListener\('focusin', handleFocusIn\)[\s\S]*?window\.removeEventListener\('focusin', handleFocusIn\)/u,
  )
  assert.match(
    dialogSource,
    /initialSvgImportMapping\(preview\.style_groups\)/u,
  )
  assert.match(
    dialogSource,
    /unresolvedSvgImportGroups\(preview\.style_groups, mapping\)/u,
  )
  assert.match(
    dialogSource,
    /const \[boundarySelection, setBoundarySelection\] = useState<[\s\S]*?>\(undefined\)/u,
  )
  assert.match(
    dialogSource,
    /boundarySelection !== undefined\s*&&\s*svgImportBoundaryIsValid\(preview, boundarySelection, mapping\)/u,
  )
  assert.match(
    dialogSource,
    /unresolved\.length === 0\s*&&\s*boundaryIsValid\s*&&\s*validationMatches\s*&&\s*boundaryConfirmed\s*&&\s*warningsAcknowledged\s*&&\s*\(!hasValidatedCuts \|\| cuttingAllowedConfirmed\)/u,
  )
  assert.match(dialogSource, /最大の輪郭を自動採用せず/u)
  assert.match(dialogSource, /formatSvgViewBox\(preview\.root_view_box\)/u)
  assert.match(dialogSource, /formatSvgPhysicalSize\(preview\.root_physical_size\)/u)
  assert.match(dialogSource, /Rust検証済みの用紙寸法:/u)
  assert.match(dialogSource, /formatSvgNumber\(validation\.width_mm\)/u)
  assert.match(dialogSource, /formatSvgNumber\(validation\.height_mm\)/u)
  assert.match(dialogSource, /<option value="">選択してください<\/option>/u)
  assert.match(dialogSource, /<option value="groups">下の線種割当で「用紙境界」を指定<\/option>/u)
  assert.match(
    dialogSource,
    /preview\.boundary_candidates\.map\(\(candidate, index\) => \(/u,
  )
  assert.match(
    dialogSource,
    /checked=\{boundaryConfirmed\}[\s\S]*?setBoundaryConfirmed\(event\.target\.checked\)/u,
  )
  assert.match(
    dialogSource,
    /setScaleInput\(event\.target\.value\)\s*invalidateValidation\(\)/u,
  )
  assert.match(
    dialogSource,
    /active === first \|\| active === dialog \|\| !dialog\.contains\(active\)/u,
  )
  assert.match(
    dialogSource,
    /preview\.style_groups\.map\(\(group, index\) => \{[\s\S]*?aria-label=\{`線種候補 \$\{index \+ 1\} の割当`\}/u,
  )
  assert.match(
    dialogSource,
    /preview\.warnings\.length === 0/u,
  )
  assert.match(
    dialogSource,
    /type="checkbox"[\s\S]*?checked=\{warningsAcknowledged\}[\s\S]*?setWarningsAcknowledged\(event\.target\.checked\)/u,
  )
  assert.match(
    dialogSource,
    /hasValidatedCuts && \([\s\S]*?checked=\{cuttingAllowedConfirmed\}[\s\S]*?setCuttingAllowedConfirmed\(event\.target\.checked\)/u,
  )
  assert.match(
    dialogSource,
    /onImport\(\{\s*importId: preview\.import_id,\s*validationId: validation\.validation_id,\s*name: name\.trim\(\),\s*mmPerUnit: scale,\s*boundaryCandidateId: boundarySelection,\s*boundaryConfirmed,\s*mappings: mapping,\s*warningsAcknowledged,\s*cuttingAllowedConfirmed,\s*\}\)/u,
  )
  assert.match(
    dialogSource,
    /className="primary" disabled=\{!canImport\} onClick=\{submit\}/u,
  )
})

function exportedFunction(name: string) {
  const start = `export function ${name}(`
  const startIndex = clientSource.indexOf(start)
  assert.ok(startIndex >= 0, `missing exported function: ${name}`)
  const nextIndex = clientSource.indexOf('\nexport function ', startIndex + start.length)
  return clientSource.slice(startIndex, nextIndex < 0 ? clientSource.length : nextIndex)
}

function appFunction(name: string, nextName: string) {
  return sourceSection(
    appSource,
    `async function ${name}(`,
    `async function ${nextName}(`,
  )
}

function sourceSection(source: string, start: string, end: string) {
  const startIndex = source.indexOf(start)
  assert.ok(startIndex >= 0, `missing section start: ${start}`)
  const endIndex = source.indexOf(end, startIndex + start.length)
  assert.ok(endIndex > startIndex, `missing section end: ${end}`)
  return source.slice(startIndex, endIndex)
}

function escapeRegExp(value: string) {
  return value.replace(/[.*+?^${}()|[\]\\]/gu, '\\$&')
}

function readSource(relativePath: string) {
  return readFileSync(new URL(relativePath, import.meta.url), 'utf8')
}
