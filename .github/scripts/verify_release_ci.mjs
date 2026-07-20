import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'

const commit = process.env.RELEASE_COMMIT
if (!/^[0-9a-f]{40}$/u.test(commit ?? '')) throw new Error('invalid release commit for CI evidence')

async function loadJson(path, url) {
  if (path) return JSON.parse(readFileSync(path, 'utf8'))
  const token = process.env.GH_TOKEN
  if (!token) throw new Error('GitHub token is required for CI evidence lookup')
  const response = await fetch(url, {
    headers: {
      authorization: `Bearer ${token}`,
      accept: 'application/vnd.github+json',
      'x-github-api-version': '2022-11-28',
    },
    redirect: 'error',
  })
  if (!response.ok) throw new Error(`GitHub CI evidence API failed: ${response.status}`)
  if ((response.headers.get('link') ?? '').includes('rel="next"')) {
    throw new Error('GitHub CI evidence exceeds the 100-item page bound')
  }
  const text = await response.text()
  if (text.length > 4_194_304) throw new Error('GitHub CI evidence exceeds the response bound')
  return JSON.parse(text)
}

async function loadArtifactBytes(path, url) {
  if (path) return readFileSync(path)
  const headers = { authorization: `Bearer ${process.env.GH_TOKEN}`, accept: 'application/vnd.github+json', 'x-github-api-version': '2022-11-28' }
  const initial = await fetch(url, { headers, redirect: 'manual' })
  if (![301, 302, 303, 307, 308].includes(initial.status)) throw new Error(`GitHub CI artifact download failed: ${initial.status}`)
  const location = new URL(initial.headers.get('location') ?? '')
  if (
    location.protocol !== 'https:' || location.port !== '' || location.username !== '' || location.password !== ''
    || !['.actions.githubusercontent.com', '.blob.core.windows.net'].some((suffix) => location.hostname.endsWith(suffix))
  ) throw new Error('GitHub CI artifact redirect is invalid')
  const response = await fetch(location, { redirect: 'error' })
  if (!response.ok) throw new Error(`GitHub CI artifact storage download failed: ${response.status}`)
  const declaredSize = Number(response.headers.get('content-length'))
  if (Number.isFinite(declaredSize) && (declaredSize < 1 || declaredSize > 16_777_216)) throw new Error('GitHub CI artifact size is outside bounds')
  const bytes = Buffer.from(await response.arrayBuffer())
  if (bytes.length < 1 || bytes.length > 16_777_216) throw new Error('GitHub CI artifact size is outside bounds')
  return bytes
}

const repo = process.env.GH_REPO
if (!process.env.WORKFLOW_RUNS_FIXTURE && !/^[^/\s]+\/[^/\s]+$/u.test(repo ?? '')) {
  throw new Error('invalid GitHub repository for CI evidence')
}
const base = `https://api.github.com/repos/${repo}`
const runs = await loadJson(
  process.env.WORKFLOW_RUNS_FIXTURE,
  `${base}/actions/workflows/ci.yml/runs?head_sha=${commit}&status=success&per_page=100`,
)
if (runs.total_count !== 1 || !Array.isArray(runs.workflow_runs) || runs.workflow_runs.length !== 1) {
  throw new Error('release commit must have exactly one successful CI workflow run')
}
const run = runs.workflow_runs[0]
const completedAt = Date.parse(run.updated_at)
if (
  !Number.isSafeInteger(run.id) || run.id < 1
  || !Number.isSafeInteger(run.check_suite_id) || run.check_suite_id < 1
  || run.head_sha !== commit
  || run.status !== 'completed'
  || run.conclusion !== 'success'
  || run.path !== '.github/workflows/ci.yml'
  || run.event !== 'push'
  || run.head_branch !== 'main'
  || !Number.isSafeInteger(run.run_attempt) || run.run_attempt < 1
  || !Number.isFinite(completedAt)
  || completedAt > Date.now() + 300_000
  || Date.now() - completedAt > 14 * 24 * 60 * 60 * 1000
) throw new Error('successful CI workflow run identity is invalid')

const checks = await loadJson(
  process.env.CHECK_RUNS_FIXTURE,
  `${base}/commits/${commit}/check-runs?per_page=100`,
)
if (
  !Number.isSafeInteger(checks.total_count)
  || checks.total_count < 1
  || checks.total_count > 100
  || !Array.isArray(checks.check_runs)
  || checks.check_runs.length !== checks.total_count
) throw new Error('CI check run set is incomplete or outside bounds')
const runMarker = `/actions/runs/${run.id}/`
const selected = checks.check_runs.filter((check) => check.details_url?.includes(runMarker))
if (selected.length < 1) throw new Error('CI workflow has no bound check runs')
const names = new Set()
const expectedNames = [
  'dependency-advisory-audit',
  'frontend',
  'macos-bundle',
  'rust (macos-latest)',
  'rust (windows-latest)',
  'slicer-acceptance',
  'windows-bundle',
]
const checkResults = selected.map((check) => {
  if (
    typeof check.name !== 'string' || check.name.length < 1 || check.name.length > 200
    || names.has(check.name)
    || check.app?.slug !== 'github-actions'
    || check.check_suite?.id !== run.check_suite_id
  ) throw new Error('CI check names are invalid or duplicated')
  names.add(check.name)
  if (check.status !== 'completed' || check.conclusion !== 'success') {
    throw new Error('CI check is incomplete or unsuccessful')
  }
  return { name: check.name, conclusion: 'success' }
}).sort((left, right) => left.name.localeCompare(right.name))
if (checkResults.map(({ name }) => name).join('\n') !== expectedNames.join('\n')) {
  throw new Error('CI required check set is incomplete or unexpected')
}

const artifacts = await loadJson(
  process.env.ARTIFACTS_FIXTURE,
  `${base}/actions/runs/${run.id}/artifacts?per_page=100`,
)
if (!Number.isSafeInteger(artifacts.total_count) || artifacts.total_count < 1 || artifacts.total_count > 100 || artifacts.artifacts?.length !== artifacts.total_count) {
  throw new Error('CI artifact set is incomplete or outside bounds')
}
const reviewArtifacts = artifacts.artifacts.filter(({ name }) => name === 'rustsec-warning-review')
if (reviewArtifacts.length !== 1) throw new Error('RustSec review artifact set is incomplete or ambiguous')
const artifact = reviewArtifacts[0]
const createdAt = Date.parse(artifact.created_at)
const expiresAt = Date.parse(artifact.expires_at)
if (
  artifact.name !== 'rustsec-warning-review'
  || !Number.isSafeInteger(artifact.id) || artifact.id < 1
  || artifact.expired !== false
  || !/^sha256:[0-9a-f]{64}$/u.test(artifact.digest ?? '')
  || !Number.isSafeInteger(artifact.size_in_bytes) || artifact.size_in_bytes < 1 || artifact.size_in_bytes > 16_777_216
  || artifact.workflow_run?.id !== run.id || artifact.workflow_run?.head_sha !== commit
  || !Number.isFinite(createdAt) || !Number.isFinite(expiresAt)
  || createdAt > Date.now() + 300_000 || expiresAt <= Date.now()
  || expiresAt - createdAt < 6 * 24 * 60 * 60 * 1000
  || expiresAt - createdAt > 8 * 24 * 60 * 60 * 1000
) throw new Error('RustSec review artifact identity or retention is invalid')
const artifactBytes = await loadArtifactBytes(
  process.env.ARTIFACT_ARCHIVE_FIXTURE,
  `${base}/actions/artifacts/${artifact.id}/zip`,
)
const archiveSha256 = createHash('sha256').update(artifactBytes).digest('hex')
if (artifactBytes.length !== artifact.size_in_bytes || `sha256:${archiveSha256}` !== artifact.digest) throw new Error('RustSec review artifact digest mismatch')

process.stdout.write(`${JSON.stringify({
  schema: 'origami2.ci-check-evidence.v1',
  sourceCommit: commit,
  workflow: '.github/workflows/ci.yml',
  workflowRunId: String(run.id),
  runAttempt: run.run_attempt,
  checkSuiteId: String(run.check_suite_id),
  checks: checkResults,
  rustsecReviewArtifact: {
    artifactId: String(artifact.id),
    name: artifact.name,
    digest: artifact.digest,
    archiveSha256,
    size: artifact.size_in_bytes,
    createdAt: artifact.created_at,
    expiresAt: artifact.expires_at,
    workflowRunId: String(run.id),
    runAttempt: run.run_attempt,
    checkSuiteId: String(run.check_suite_id),
  },
})}\n`)
