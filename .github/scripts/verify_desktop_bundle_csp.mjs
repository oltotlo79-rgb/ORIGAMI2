import { readFileSync, statSync } from 'node:fs'
import { basename, join, resolve } from 'node:path'

const directory = resolve(process.argv[2] ?? '')
const htmlPath = join(directory, 'index.html')
if (statSync(htmlPath).size > 65_536) throw new Error('desktop bundle HTML is oversized')
const html = readFileSync(htmlPath, 'utf8')
if (/<style\b|<script\b(?![^>]*\bsrc=)|\sstyle\s*=|(?:data|blob|https?):/iu.test(html)) {
  throw new Error('desktop bundle HTML requires forbidden inline or remote CSP authority')
}
const scripts = [...html.matchAll(/<script type="module" crossorigin src="(\/assets\/[A-Za-z0-9_-]+\.js)"><\/script>/gu)]
const styles = [...html.matchAll(/<link rel="stylesheet" crossorigin href="(\/assets\/[A-Za-z0-9_-]+\.css)">/gu)]
if (scripts.length !== 1 || styles.length !== 1) throw new Error('desktop bundle entrypoints are ambiguous')
for (const reference of [scripts[0][1], styles[0][1]]) {
  const path = join(directory, reference.slice(1))
  if (!statSync(path).isFile() || basename(path) !== basename(reference)) {
    throw new Error('desktop bundle entrypoint is not a regular local asset')
  }
}
const css = readFileSync(join(directory, styles[0][1].slice(1)), 'utf8')
if (/url\(\s*["']?(?:data|blob|https?):/iu.test(css) || /@import\b/iu.test(css)) {
  throw new Error('desktop bundle CSS contains remote, blob, data, or imported content')
}
