import { readFileSync, statSync } from 'node:fs'

const path = process.argv[2]
const status = process.argv[3]
if (status !== '200') throw new Error('release API status is not 200')
if (statSync(path).size > 65_536) throw new Error('release API headers are oversized')
const bytes = readFileSync(path)
for (const byte of bytes) {
  if (byte > 0x7e || (byte < 0x20 && byte !== 0x0d && byte !== 0x0a)) {
    throw new Error('release API headers contain control or non-ASCII bytes')
  }
}
const raw = bytes.toString('ascii')
if (!raw.endsWith('\r\n\r\n') || raw.replace(/\r\n/gu, '').includes('\n')) {
  throw new Error('release API header framing is invalid')
}
const lines = raw.slice(0, -4).split('\r\n')
if (lines.filter((line) => /^HTTP\//u.test(line)).length !== 1
  || !/^HTTP\/(?:1\.1|2|3) 200(?: [A-Za-z ]+)?$/u.test(lines[0])) {
  throw new Error('release API has multiple, provisional, or invalid status lines')
}
const headers = new Map()
for (const line of lines.slice(1)) {
  if (/^[ \t]/u.test(line)) throw new Error('release API obs-fold is forbidden')
  const match = /^([!#$%&'*+\-.^_|~0-9A-Za-z]+): ([\x20-\x7e]*)$/u.exec(line)
  if (!match) throw new Error('release API header syntax is invalid')
  const name = match[1].toLowerCase()
  if (headers.has(name)) throw new Error('duplicate release API header: ' + name)
  headers.set(name, match[2])
}
const contentType = headers.get('content-type') ?? ''
if (!/^application\/(?:json|vnd\.github\+json)(?:; charset=utf-8)?$/iu.test(contentType)) {
  throw new Error('release API content type is invalid')
}
if (!/^"[A-Za-z0-9+/=_:.-]{1,256}"$/u.test(headers.get('etag') ?? '')) {
  throw new Error('release API ETag is missing, weak, or invalid')
}
const cacheControl = (headers.get('cache-control') ?? '').toLowerCase()
if (!/(?:^|,\s*)(?:private|no-store)(?:,|$)/u.test(cacheControl)
  || /(?:^|,\s*)public(?:,|$)|s-maxage=[1-9]/u.test(cacheControl)) {
  throw new Error('release API cache policy is unsafe')
}
if (!/^[1-9][0-9]*$/u.test(headers.get('x-ratelimit-remaining') ?? '')) {
  throw new Error('release API rate limit is exhausted or invalid')
}
if (headers.has('link') || headers.has('retry-after') || /^[1-9][0-9]*$/u.test(headers.get('age') ?? '')) {
  throw new Error('release API response is paginated, delayed, or cached')
}
