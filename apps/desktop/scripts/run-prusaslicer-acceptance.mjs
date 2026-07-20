import { execFileSync, spawnSync } from 'node:child_process'
import { mkdtempSync, readFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const desktop = resolve(fileURLToPath(new URL('..', import.meta.url)))
const workspace = resolve(desktop, '..', '..')
const scratch = mkdtempSync(join(tmpdir(), 'origami2-prusaslicer-'))
const artifacts = process.env.ORIGAMI2_PRUSASLICER_ARTIFACTS
  ? resolve(process.env.ORIGAMI2_PRUSASLICER_ARTIFACTS)
  : scratch
const slicer = process.env.PRUSASLICER_BIN

if (!slicer) throw new Error('PRUSASLICER_BIN must point to the pinned CLI executable')

const invoke = (args) => {
  const result = spawnSync(slicer, args, {
    cwd: workspace,
    encoding: 'utf8',
    maxBuffer: 16 * 1024 * 1024,
  })
  if (result.error) throw result.error
  return result
}

const cleanSuccess = (result, operation) => {
  if (result.status !== 0) {
    throw new Error(`${operation} exited ${result.status}\n${result.stdout}\n${result.stderr}`)
  }
  if (result.stderr.trim()) throw new Error(`${operation} wrote stderr:\n${result.stderr}`)
  if (/\[(?:error|warning)\]/iu.test(result.stdout)) {
    throw new Error(`${operation} logged an error or warning:\n${result.stdout}`)
  }
}

const fields = (text) => Object.fromEntries(
  [...text.matchAll(/^([a-z_]+)\s*=\s*(.+)$/gmu)].map((match) => [match[1], match[2].trim()]),
)

try {
  if (!process.env.ORIGAMI2_PRUSASLICER_ARTIFACTS) {
    execFileSync('cargo', [
      'run', '--quiet', '--locked', '--release', '-p', 'ori-formats',
      '--example', 'generate_gltf_validator_fixtures', '--', artifacts,
    ], { cwd: workspace, stdio: 'inherit' })
  }

  const solid = join(artifacts, 'positive-thickness.stl')
  const infoResult = invoke(['--loglevel', '2', '--info', solid])
  cleanSuccess(infoResult, 'positive-thickness STL analysis')
  const info = fields(infoResult.stdout)
  const exact = {
    size_x: 10, size_y: 10, size_z: 2,
    min_x: 0, min_y: 0, min_z: 0,
    max_x: 10, max_y: 10, max_z: 2,
    number_of_facets: 12, number_of_parts: 1,
  }
  for (const [key, expected] of Object.entries(exact)) {
    if (Math.abs(Number(info[key]) - expected) > 1e-5) {
      throw new Error(`positive-thickness STL: ${key}=${info[key]}, expected ${expected}`)
    }
  }
  if (info.manifold !== 'yes') throw new Error('positive-thickness STL is not manifold')
  if (Math.abs(Number(info.volume) - 200) > 0.001) {
    throw new Error(`positive-thickness STL: volume=${info.volume}, expected 200 mm3`)
  }
  const repairs = ['facets_added', 'facets_removed', 'facets_reversed', 'backwards_edges']
  if (repairs.some((key) => Number(info[key] ?? 0) !== 0)) {
    throw new Error(`positive-thickness STL was repaired:\n${infoResult.stdout}`)
  }

  const open = invoke(['--loglevel', '2', '--info', join(artifacts, 'static.stl')])
  if (open.status === 0 || !/empty file|loading of a model file failed/iu.test(open.stdout + open.stderr)) {
    throw new Error(`open sheet was not rejected:\n${open.stdout}\n${open.stderr}`)
  }
  const unproven = invoke([
    '--loglevel', '2', '--info', join(artifacts, 'unproven-nonmanifold.stl'),
  ])
  if (unproven.status !== 0 || Number(fields(unproven.stdout).facets_removed) < 1) {
    throw new Error(`unproven non-manifold mesh did not require repair:\n${unproven.stdout}`)
  }

  const gcode = join(scratch, 'positive-thickness.gcode')
  const slice = invoke([
    '--loglevel', '2',
    '--layer-height', '0.3',
    '--first-layer-height', '0.35',
    '--export-gcode', '--output', gcode, solid,
  ])
  cleanSuccess(slice, 'positive-thickness G-code export')
  const text = readFileSync(gcode, 'utf8')
  const layers = [...text.matchAll(/^;LAYER_CHANGE$/gmu)].length
  if (layers !== 6) throw new Error(`G-code layer count=${layers}, expected 6`)
  const moves = [...text.matchAll(/^G[01]\s+(.+)$/gmu)].flatMap((line) =>
    [...line[1].matchAll(/([XYZ])(-?(?:\d+(?:\.\d*)?|\.\d+))/gu)]
      .map((axis) => [axis[1], Number(axis[2])]))
  if (moves.length === 0 || moves.some(([, value]) => !Number.isFinite(value))) {
    throw new Error('G-code motion bounds are missing or non-finite')
  }
  const layerZ = [...text.matchAll(/^;Z:(-?(?:\d+(?:\.\d*)?|\.\d+))$/gmu)]
    .map((match) => Number(match[1]))
  if (layerZ.length !== layers || layerZ.some((value) => !Number.isFinite(value))
    || Math.min(...layerZ) < 0 || Math.max(...layerZ) > 2.001) {
    throw new Error(`G-code layer bounds are invalid: ${layerZ.join(', ')}`)
  }
  console.log(
    'PrusaSlicer acceptance: manifold=yes, bounds=10x10x2 mm, '
    + 'volume=200 mm3, triangles=12, repairs=0, layers=6',
  )
  console.log('Expected failures: open sheet rejected; unproven mesh required repair')
} finally {
  rmSync(scratch, { recursive: true, force: true })
}
