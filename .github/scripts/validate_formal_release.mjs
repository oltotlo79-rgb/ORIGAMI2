import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'

const mode = process.env.REQUESTED_MODE
const requestedTag = process.env.REQUESTED_TAG
if (!['dry-run', 'prerelease', 'stable', 'promote'].includes(mode)) {
  throw new Error(`unsupported release mode: ${mode}`)
}
const config = JSON.parse(readFileSync('apps/desktop/src-tauri/tauri.conf.json', 'utf8'))
const version = config.version
if (
  typeof version !== 'string'
  || !/^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$/u.test(version)
) throw new Error(`invalid application version: ${version}`)
const cargoManifest = readFileSync('Cargo.toml', 'utf8')
const cargoVersion = /^\[workspace\.package\][\s\S]*?^version = "([^"]+)"$/mu.exec(cargoManifest)?.[1]
if (cargoVersion !== version) {
  throw new Error(`Cargo workspace version ${cargoVersion} does not match application ${version}`)
}
let tag = ''
const head = execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim()
if (mode === 'dry-run') {
  if (requestedTag !== undefined && requestedTag !== '') {
    throw new Error('dry-run must not select a release tag')
  }
} else {
  tag = requestedTag
  if (tag !== `v${version}`) throw new Error(`tag ${tag} does not match application v${version}`)
  execFileSync('git', ['verify-tag', tag], { stdio: 'inherit' })
  const tagCommit = execFileSync('git', ['rev-list', '-n', '1', tag], { encoding: 'utf8' }).trim()
  if (tagCommit !== head) throw new Error(`signed tag ${tag} does not resolve to HEAD`)
}
const output = process.env.GITHUB_OUTPUT
if (output) {
  const { appendFileSync } = await import('node:fs')
  appendFileSync(output, `mode=${mode}\ntag=${tag}\nversion=${version}\ncommit=${head}\n`)
}
console.log(`formal release contract: mode=${mode}, tag=${tag || '(dry-run)'}, version=${version}`)
