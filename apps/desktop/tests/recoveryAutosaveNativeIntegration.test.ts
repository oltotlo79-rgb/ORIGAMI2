import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const recoverySource = read('../src-tauri/src/recovery.rs')
const nativeRoot = read('../src-tauri/src/lib.rs')

test('native autosave health wire exposes only status and a non-wrapping transition ID', () => {
  const dto = section(
    recoverySource,
    'pub(super) enum RecoveryAutosaveHealthStatus',
    '/// `restore_recovery` accepts',
  )
  for (const required of [
    'PendingFirstAttempt',
    'Operational',
    'PersistenceFailed',
    'schema_version: u32',
    'status: RecoveryAutosaveHealthStatus',
    'transition_id: u32',
  ]) {
    assert.ok(dto.includes(required), `missing health contract: ${required}`)
  }
  assert.doesNotMatch(
    dto,
    /(?:path|error|generation|project_id|revision|document)/u,
  )

  const transition = section(
    recoverySource,
    'fn record_autosave_health',
    '\n}\n\nstruct RecoveryRuntimeInner',
  )
  assert.match(transition, /checked_add\(1\)/u)
  assert.match(transition, /autosave_health_transition_id == u32::MAX/u)
  assert.match(
    transition,
    /next == u32::MAX[\s\S]*RecoveryAutosaveHealthStatus::PersistenceFailed/u,
  )
})

test('every timer result is reduced to cached health before the internal result is discarded', () => {
  const tick = section(
    recoverySource,
    'fn run_recovery_autosave_tick',
    'pub(super) fn start_recovery_autosave_timer',
  )
  assert.match(tick, /recovery\.record_autosave_observation\(&outcome\)/u)
  const observation = section(
    recoverySource,
    'fn record_autosave_observation',
    '/// Performs bounded file reinspection',
  )
  assert.match(
    observation,
    /Stored[\s\S]*Cleared[\s\S]*Duplicate[\s\S]*RecoveryAutosaveHealthStatus::Operational/u,
  )
  assert.match(
    observation,
    /Err\(_\) => RecoveryAutosaveHealthStatus::PersistenceFailed/u,
  )
  assert.match(
    observation,
    /StartupDecisionPending[\s\S]*AutomaticWritesStopped[\s\S]*Superseded[\s\S]*=> return/u,
  )
})

test('the read-only status command is registered and returns only the fixed redacted failure', () => {
  const command = section(
    recoverySource,
    'pub(super) fn get_recovery_autosave_status',
    '#[tauri::command]\npub(super) async fn restore_recovery',
  )
  assert.match(command, /autosave_health_response\(\)/u)
  assert.match(command, /RECOVERY_COMMAND_FAILED_MESSAGE\.to_owned\(\)/u)
  assert.doesNotMatch(command, /format!|to_string\(\)|slot_path|RecoveryStorageError::/u)
  const recoveryImports = section(
    nativeRoot,
    'use recovery::{',
    '};\nuse serde',
  )
  assert.match(recoveryImports, /\bget_recovery_autosave_status\b/u)
  assert.equal(
    nativeRoot.match(/^\s*get_recovery_autosave_status,\s*$/gmu)?.length,
    1,
  )
})

function read(relativePath: string) {
  return readFileSync(new URL(relativePath, import.meta.url), 'utf8')
}

function section(source: string, start: string, end: string) {
  const startIndex = source.indexOf(start)
  assert.ok(startIndex >= 0, `missing section start: ${start}`)
  const endIndex = source.indexOf(end, startIndex + start.length)
  assert.ok(endIndex > startIndex, `missing section end: ${end}`)
  return source.slice(startIndex, endIndex)
}
