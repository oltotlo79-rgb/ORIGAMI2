import { execFileSync } from 'node:child_process'
import { lstatSync, readFileSync, realpathSync } from 'node:fs'
import { isAbsolute, relative, resolve, sep } from 'node:path'

const [statusInput, manifestInput, idsInput] = process.argv.slice(2)
const fail = (message) => { throw new Error(`requirements traceability: ${message}`) }
if (!statusInput || !manifestInput || !idsInput || !process.env.REQUIREMENTS_EVIDENCE_ROOT) fail('root and three input files are required')
const root = realpathSync(process.env.REQUIREMENTS_EVIDENCE_ROOT)
try {
  if (execFileSync('git', ['-C', root, 'rev-parse', '--is-shallow-repository'], { encoding: 'utf8' }).trim() !== 'false') fail('shallow repositories are unsupported')
} catch (error) { if (error.message?.startsWith('requirements traceability:')) throw error; fail('repository identity is unavailable') }

const safeFile = (input) => {
  if (typeof input !== 'string' || input.length < 1 || input.length > 240 || isAbsolute(input) || input.includes('\\')) fail('path is unsafe')
  const candidate = resolve(root, input)
  const rel = relative(root, candidate)
  if (!rel || rel.startsWith('..') || isAbsolute(rel)) fail('path escapes repository root')
  let current = root
  for (const component of rel.split(sep)) {
    current = resolve(current, component)
    const stat = lstatSync(current)
    if (stat.isSymbolicLink()) fail(`path contains a symbolic link or junction: ${input}`)
  }
  const stat = lstatSync(candidate)
  if (!stat.isFile()) fail(`path is not a regular file: ${input}`)
  const realRel = relative(root, realpathSync(candidate))
  if (!realRel || realRel.startsWith('..') || isAbsolute(realRel)) fail(`path resolves outside repository: ${input}`)
  return candidate
}
const boundedText = (input, maximum) => {
  const path = safeFile(input)
  if (lstatSync(path).size > maximum) fail(`file exceeds ${maximum} bytes: ${input}`)
  return readFileSync(path, 'utf8')
}
const exactKeys = (value, keys, label) => {
  if (!value || typeof value !== 'object' || Array.isArray(value)) fail(`${label} must be an object`)
  if (Object.keys(value).sort().join('\0') !== [...keys].sort().join('\0')) fail(`${label} has an invalid key set`)
}

const status = boundedText(statusInput, 2_000_000)
const expectedIds = boundedText(idsInput, 20_000).trimEnd().split('\n')
if (expectedIds.length < 1 || expectedIds.length > 500 || expectedIds.some((id) => !/^[A-Z]{2,4}-\d{3}$/u.test(id)) || new Set(expectedIds).size !== expectedIds.length || [...expectedIds].sort().join('\0') !== expectedIds.join('\0')) fail('requirement ID contract is invalid')
const rows = [...status.matchAll(/^\|\s*([A-Z]{2,4}-\d{3})\s*\|\s*([^|]+?)\s*\|/gmu)]
const statuses = new Map()
for (const [, id, stateValue] of rows) {
  const state = stateValue.trim()
  if (!['実装済み', '部分実装', '未着手'].includes(state)) fail(`invalid requirement status: ${id}`)
  if (statuses.has(id)) fail(`duplicate requirement row: ${id}`)
  statuses.set(id, state)
}
if ([...statuses.keys()].sort().join('\0') !== expectedIds.join('\0')) fail('status table does not match the requirement ID contract')

const manifestText = boundedText(manifestInput, 2_000_000)
const manifest = JSON.parse(manifestText)
if (`${JSON.stringify(manifest, null, 2)}\n` !== manifestText) fail('manifest JSON is non-canonical or contains duplicate keys')
exactKeys(manifest, ['schema', 'requirements'], 'manifest')
if (manifest.schema !== 'origami2.requirements-evidence.v1') fail('unsupported schema')
if (!Array.isArray(manifest.requirements) || manifest.requirements.length !== expectedIds.length) fail('manifest does not cover the exact requirement set')
const seen = new Set()
for (const entry of manifest.requirements) {
  exactKeys(entry, ['id', 'status', 'commits', 'evidence', 'limitations', 'missingAcceptance'], 'requirement entry')
  if (!statuses.has(entry.id) || seen.has(entry.id)) fail(`unknown or duplicate requirement: ${entry.id}`)
  seen.add(entry.id)
  if (entry.status !== statuses.get(entry.id)) fail(`status mismatch: ${entry.id}`)
  if (!Array.isArray(entry.commits) || entry.commits.length > 32 || new Set(entry.commits).size !== entry.commits.length) fail(`invalid commits: ${entry.id}`)
  for (const commit of entry.commits) {
    if (!/^[0-9a-f]{40}$/u.test(commit)) fail(`commit id must be a full SHA-1: ${entry.id}`)
    try { execFileSync('git', ['-C', root, 'merge-base', '--is-ancestor', commit, 'HEAD'], { stdio: 'ignore' }) } catch { fail(`commit is not an ancestor of HEAD: ${entry.id}`) }
  }
  if (!Array.isArray(entry.evidence) || entry.evidence.length > 32) fail(`invalid evidence count: ${entry.id}`)
  const identities = new Set()
  const kinds = new Set()
  for (const evidence of entry.evidence) {
    exactKeys(evidence, ['kind', 'path', 'selector'], `evidence for ${entry.id}`)
    if (!['production-symbol', 'test', 'contract', 'documentation'].includes(evidence.kind)) fail(`invalid evidence kind: ${entry.id}`)
    if (typeof evidence.selector !== 'string' || evidence.selector.length < 3 || evidence.selector.length > 300 || /[\r\n\0]/u.test(evidence.selector)) fail(`invalid evidence selector: ${entry.id}`)
    const identity = `${evidence.path}\0${evidence.selector}`
    if (identities.has(identity)) fail(`duplicate or relabeled evidence: ${entry.id}`)
    identities.add(identity)
    kinds.add(evidence.kind)
    if (!boundedText(evidence.path, 5_000_000).includes(evidence.selector)) fail(`selector is absent: ${entry.id} ${evidence.path}`)
    let historicallyBound = false
    for (const commit of entry.commits) {
      try {
        const historical = execFileSync('git', ['-C', root, 'show', `${commit}:${evidence.path}`], { encoding: 'utf8', maxBuffer: 5_000_000, stdio: ['ignore', 'pipe', 'ignore'] })
        if (historical.includes(evidence.selector)) { historicallyBound = true; break }
      } catch { /* Another listed commit may bind this evidence. */ }
    }
    if (!historicallyBound && evidence.kind !== 'documentation') fail(`selector is not bound to a listed commit: ${entry.id}`)
  }
  for (const [label, values] of [['limitations', entry.limitations], ['missingAcceptance', entry.missingAcceptance]]) {
    if (!Array.isArray(values) || values.length > 16 || values.some((value) => typeof value !== 'string' || value.length < 3 || value.length > 500)) fail(`invalid ${label}: ${entry.id}`)
  }
  if (entry.status === '実装済み') {
    if (entry.commits.length < 1 || !kinds.has('production-symbol') || (!kinds.has('test') && !kinds.has('contract'))) fail(`implemented requirement lacks production and executable evidence: ${entry.id}`)
    if (entry.limitations.length || entry.missingAcceptance.length) fail(`implemented requirement carries partial-only fields: ${entry.id}`)
  } else if (entry.status === '部分実装') {
    if (entry.commits.length < 1 || !kinds.has('production-symbol') || (!kinds.has('test') && !kinds.has('contract')) || entry.limitations.length < 1 || entry.missingAcceptance.length < 1) fail(`partial requirement lacks explicit present and missing boundaries: ${entry.id}`)
  } else if (entry.commits.length || entry.evidence.some(({ kind }) => kind !== 'documentation') || entry.limitations.length < 1 || entry.missingAcceptance.length < 1) fail(`unstarted requirement has implementation evidence or lacks boundaries: ${entry.id}`)
}
if ([...seen].sort().join('\0') !== expectedIds.join('\0')) fail('manifest requirement set is incomplete')
process.stdout.write(`${seen.size}\n`)
