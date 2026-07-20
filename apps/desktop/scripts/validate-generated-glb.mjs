import { execFileSync } from 'node:child_process'
import { mkdtempSync, readFileSync, readdirSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import validator from 'gltf-validator'

const desktop = resolve(fileURLToPath(new URL('..', import.meta.url)))
const workspace = resolve(desktop, '..', '..')
const output = mkdtempSync(join(tmpdir(), 'origami2-gltf-validator-'))

try {
  execFileSync(
    'cargo',
    [
      'run', '--quiet', '--locked', '-p', 'ori-formats',
      '--example', 'generate_gltf_validator_fixtures', '--', output,
    ],
    { cwd: workspace, stdio: 'inherit' },
  )
  const files = readdirSync(output).filter((file) => file.endsWith('.glb')).sort()
  if (files.join(',') !== 'animated.glb,static.glb,textured.glb') {
    throw new Error(`unexpected generated fixture set: ${files.join(',')}`)
  }
  for (const file of files) {
    const report = await validator.validateBytes(
      new Uint8Array(readFileSync(join(output, file))),
      { uri: file, maxIssues: 1000 },
    )
    if (report.issues.numErrors !== 0) {
      console.error(JSON.stringify(report, null, 2))
      throw new Error(`${file}: Khronos glTF Validator reported errors`)
    }
    console.log(
      `${file}: error 0, warning ${report.issues.numWarnings}, info ${report.issues.numInfos}`,
    )
  }
} finally {
  rmSync(output, { recursive: true, force: true })
}
