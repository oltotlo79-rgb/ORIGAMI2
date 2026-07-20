import { execFileSync, spawnSync } from 'node:child_process'
import { mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const desktop = resolve(fileURLToPath(new URL('..', import.meta.url)))
const workspace = resolve(desktop, '..', '..')
const scratch = mkdtempSync(join(tmpdir(), 'origami2-blender-'))
const blender = process.env.BLENDER_BIN

if (!blender) throw new Error('BLENDER_BIN must point to the pinned Blender executable')

try {
  execFileSync('cargo', [
    'run', '--quiet', '--locked', '--release', '-p', 'ori-formats',
    '--example', 'generate_gltf_validator_fixtures', '--', scratch,
  ], { cwd: workspace, stdio: 'inherit' })

  const result = spawnSync(blender, [
    '--background', '--factory-startup', '--disable-autoexec',
    '--python-exit-code', '1',
    '--python', join(desktop, 'scripts', 'blender-import-acceptance.py'),
    '--', scratch,
  ], { cwd: workspace, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 })
  if (result.error) throw result.error
  if (result.status !== 0) {
    throw new Error(`Blender exited ${result.status}\n${result.stdout}\n${result.stderr}`)
  }
  if (result.stderr.trim()) throw new Error(`Blender wrote stderr:\n${result.stderr}`)
  if (!result.stdout.includes('ORIGAMI2_BLENDER_ACCEPTANCE=')) {
    throw new Error(`Blender acceptance report missing:\n${result.stdout}`)
  }
  process.stdout.write(result.stdout)
} finally {
  rmSync(scratch, { recursive: true, force: true })
}
