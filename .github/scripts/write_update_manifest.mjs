import { createHash } from 'node:crypto'
import { readFileSync, writeFileSync } from 'node:fs'
import { basename, join, resolve } from 'node:path'

const directoryArgument = process.argv[2]
if (
  typeof directoryArgument !== 'string'
  || directoryArgument.length < 1
  || directoryArgument.length > 4096
  || /[\u0000-\u001f\u007f*?\[\]]/u.test(directoryArgument)
  || directoryArgument.startsWith('-')
) throw new Error('invalid update manifest directory path')
const directory = resolve(directoryArgument)
const platform = process.env.PLATFORM
const version = process.env.VERSION
const signaturePolicy = process.env.SIGNATURE_POLICY
if (!['windows-x64', 'macos-arm64'].includes(platform)) {
  throw new Error('unsupported update manifest platform')
}
if (!/^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$/u.test(version ?? '')) {
  throw new Error('invalid update manifest version')
}
if (!['platform-signed', 'unsigned-dry-run'].includes(signaturePolicy)) {
  throw new Error('invalid update manifest signature policy')
}
const prefix = `ORIGAMI2-v${version}-${platform}`
const names = platform === 'windows-x64'
  ? [`${prefix}-portable.zip`, `${prefix}-setup.exe`, `${prefix}.cdx.json`]
  : [`${prefix}-app.tar.gz`, `${prefix}.cdx.json`]
const assets = names.sort().map((name) => Object.freeze({
  name,
  sha256: createHash('sha256').update(readFileSync(join(directory, name))).digest('hex'),
}))
const manifest = {
  schema: 'origami2.update-manifest.v1',
  version,
  platform,
  signaturePolicy,
  assets,
}
writeFileSync(
  join(directory, `${prefix}.update.json`),
  `${JSON.stringify(manifest)}\n`,
  { encoding: 'utf8', flag: 'wx' },
)
console.log(`wrote canonical update manifest for ${basename(directory)} ${platform}`)
