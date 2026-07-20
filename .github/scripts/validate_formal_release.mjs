import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'

const mode = process.env.REQUESTED_MODE
const requestedTag = process.env.REQUESTED_TAG
if (!['dry-run', 'prerelease', 'stable', 'promote'].includes(mode)) {
  throw new Error(`unsupported release mode: ${mode}`)
}
const config = JSON.parse(readFileSync('apps/desktop/src-tauri/tauri.conf.json', 'utf8'))
const version = config.version
if (!/^\d+\.\d+\.\d+$/u.test(version)) throw new Error(`invalid application version: ${version}`)
let tag = ''
if (mode !== 'dry-run') {
  tag = requestedTag
  if (tag !== `v${version}`) throw new Error(`tag ${tag} does not match application v${version}`)
  execFileSync('git', ['verify-tag', tag], { stdio: 'inherit' })
  const tagCommit = execFileSync('git', ['rev-list', '-n', '1', tag], { encoding: 'utf8' }).trim()
  const head = execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim()
  if (tagCommit !== head) throw new Error(`signed tag ${tag} does not resolve to HEAD`)
}
const output = process.env.GITHUB_OUTPUT
if (output) {
  const { appendFileSync } = await import('node:fs')
  appendFileSync(output, `mode=${mode}\ntag=${tag}\nversion=${version}\n`)
}
console.log(`formal release contract: mode=${mode}, tag=${tag || '(dry-run)'}, version=${version}`)
