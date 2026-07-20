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
if (
  !Number.isSafeInteger(run.id) || run.id < 1
  || run.head_sha !== commit
  || run.status !== 'completed'
  || run.conclusion !== 'success'
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
const checkResults = selected.map((check) => {
  if (
    typeof check.name !== 'string' || check.name.length < 1 || check.name.length > 200
    || names.has(check.name)
  ) throw new Error('CI check names are invalid or duplicated')
  names.add(check.name)
  if (check.status !== 'completed' || check.conclusion !== 'success') {
    throw new Error('CI check is incomplete or unsuccessful')
  }
  return { name: check.name, conclusion: 'success' }
}).sort((left, right) => left.name.localeCompare(right.name))

process.stdout.write(`${JSON.stringify({
  schema: 'origami2.ci-check-evidence.v1',
  sourceCommit: commit,
  workflow: '.github/workflows/ci.yml',
  workflowRunId: String(run.id),
  checks: checkResults,
})}\n`)
