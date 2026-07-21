import { closeSync, fstatSync, lstatSync, openSync, readFileSync, realpathSync } from 'node:fs'
import { basename, dirname, join, resolve } from 'node:path'

const directory = resolve(process.argv[2] ?? '')
if (!lstatSync(directory).isDirectory() || realpathSync(directory) !== directory) {
  throw new Error('desktop bundle root is not a canonical directory')
}
const htmlPath = join(directory, 'index.html')
const html = readPinnedFile(htmlPath, 65_536)
if (/<style\b|<script\b(?![^>]*\bsrc=)|\sstyle\s*=|\ssrcset\s*=|<base\b|<meta\b[^>]*http-equiv\s*=\s*["']?refresh|(?:data|blob|https?):|\b(?:src|href)\s*=\s*["']\/\//iu.test(html)) {
  throw new Error('desktop bundle HTML requires forbidden inline or remote CSP authority')
}
const scripts = [...html.matchAll(/<script type="module" crossorigin src="(\/assets\/[A-Za-z0-9_-]+\.js)"><\/script>/gu)]
const styles = [...html.matchAll(/<link rel="stylesheet" crossorigin href="(\/assets\/[A-Za-z0-9_-]+\.css)">/gu)]
if (scripts.length !== 1 || styles.length !== 1 || (html.match(/<script\b/gu)?.length ?? 0) !== 1) throw new Error('desktop bundle entrypoints are ambiguous')
let css = ''
let javascript = ''
for (const reference of [scripts[0][1], styles[0][1]]) {
  const path = join(directory, reference.slice(1))
  if (realpathSync(dirname(path)) !== join(directory, 'assets') || basename(path) !== basename(reference)) {
    throw new Error('desktop bundle entrypoint is not a regular local asset')
  }
  const content = readPinnedFile(path, reference.endsWith('.css') ? 2_097_152 : 8_388_608)
  if (reference.endsWith('.css')) css = content
  else javascript = content
}
const canonicalCss = decodeCssEscapes(css.replace(/\/\*[\s\S]*?\*\//gu, ''))
if (/url\(\s*["']?(?:data|blob|https?):/iu.test(canonicalCss) || /@import\b/iu.test(canonicalCss)) {
  throw new Error('desktop bundle CSS contains remote, blob, data, or imported content')
}
if (/\b(?:importScripts|WebSocket|EventSource)\s*\(|\bimport\s*\(|[#@]\s*sourceMappingURL\s*=|\bfetch\s*\(\s*["'`]https?:\/\//u.test(javascript)) {
  throw new Error('desktop bundle JavaScript contains forbidden dynamic loading or network authority')
}
for (const link of html.matchAll(/<link\b[^>]*\brel="(modulepreload|preload)"[^>]*\bhref="([^"]+)"[^>]*>/gu)) {
  if (!/^\/assets\/[A-Za-z0-9_-]+\.(?:js|css|woff2)$/u.test(link[2])) {
    throw new Error('desktop bundle preload is not a canonical local asset')
  }
}

function readPinnedFile(path, limit) {
  const link = lstatSync(path)
  if (link.isSymbolicLink()) throw new Error('desktop bundle symlink is forbidden')
  const descriptor = openSync(path, 'r')
  try {
    const before = fstatSync(descriptor)
    if (!before.isFile() || before.nlink !== 1 || before.size > limit) throw new Error('desktop bundle file identity is invalid')
    const value = readFileSync(descriptor, 'utf8')
    const after = fstatSync(descriptor)
    const finalLink = lstatSync(path)
    if (before.dev !== after.dev || before.ino !== after.ino || before.size !== after.size
      || before.mtimeMs !== after.mtimeMs || finalLink.dev !== before.dev || finalLink.ino !== before.ino) {
      throw new Error('desktop bundle file changed during verification')
    }
    return value
  } finally {
    closeSync(descriptor)
  }
}

function decodeCssEscapes(value) {
  return value.replace(/\\([0-9a-fA-F]{1,6})[\t\n\r\f ]?|\\([^\n\r\f0-9a-fA-F])/gu, (_match, hexadecimal, escaped) =>
    hexadecimal === undefined ? escaped : String.fromCodePoint(Number.parseInt(hexadecimal, 16)))
}
