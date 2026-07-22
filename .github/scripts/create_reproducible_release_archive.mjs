import { createHash } from 'node:crypto'
import { lstatSync, readFileSync, readdirSync, writeFileSync } from 'node:fs'
import { basename, join, resolve } from 'node:path'
import { gzipSync } from 'node:zlib'

const [platform, outputArgument, releaseRootArgument] = process.argv.slice(2)
if (!['windows-x64', 'macos-arm64'].includes(platform)) throw new Error('unsupported archive platform')
const output = resolve(outputArgument ?? '')
const releaseRoot = resolve(releaseRootArgument ?? '')

function crc32(bytes) {
  let crc = 0xffffffff
  for (const byte of bytes) {
    crc ^= byte
    for (let bit = 0; bit < 8; bit += 1) crc = (crc >>> 1) ^ (0xedb88320 & -(crc & 1))
  }
  return (crc ^ 0xffffffff) >>> 0
}

function filesBelow(root, prefix) {
  const result = []
  const visit = (directory, relative) => {
    for (const name of readdirSync(directory).sort()) {
      const path = join(directory, name)
      const stat = lstatSync(path)
      if (stat.isSymbolicLink()) throw new Error('release archives forbid symbolic links')
      const child = relative ? `${relative}/${name}` : name
      if (stat.isDirectory()) visit(path, child)
      else if (stat.isFile()) result.push({ name: `${prefix}${child}`, bytes: readFileSync(path), executable: (stat.mode & 0o111) !== 0 })
      else throw new Error('release archives accept only regular files and directories')
    }
  }
  visit(root, '')
  return result
}

function zip(entries) {
  const local = []
  const central = []
  let offset = 0
  for (const entry of entries) {
    const name = Buffer.from(entry.name)
    const crc = crc32(entry.bytes)
    const header = Buffer.alloc(30)
    header.writeUInt32LE(0x04034b50, 0); header.writeUInt16LE(20, 4)
    header.writeUInt16LE(0x0800, 6); header.writeUInt16LE(0, 8)
    header.writeUInt16LE(0, 10); header.writeUInt16LE(0x0021, 12)
    header.writeUInt32LE(crc, 14); header.writeUInt32LE(entry.bytes.length, 18)
    header.writeUInt32LE(entry.bytes.length, 22); header.writeUInt16LE(name.length, 26)
    local.push(header, name, entry.bytes)
    const directory = Buffer.alloc(46)
    directory.writeUInt32LE(0x02014b50, 0); directory.writeUInt16LE(0x0314, 4)
    directory.writeUInt16LE(20, 6); directory.writeUInt16LE(0x0800, 8)
    directory.writeUInt16LE(0, 10); directory.writeUInt16LE(0, 12)
    directory.writeUInt16LE(0x0021, 14); directory.writeUInt32LE(crc, 16)
    directory.writeUInt32LE(entry.bytes.length, 20); directory.writeUInt32LE(entry.bytes.length, 24)
    directory.writeUInt16LE(name.length, 28)
    directory.writeUInt32LE(((entry.executable ? 0o100755 : 0o100644) << 16) >>> 0, 38)
    directory.writeUInt32LE(offset, 42); central.push(directory, name)
    offset += header.length + name.length + entry.bytes.length
  }
  const centralBytes = Buffer.concat(central)
  const end = Buffer.alloc(22)
  end.writeUInt32LE(0x06054b50, 0); end.writeUInt16LE(entries.length, 8)
  end.writeUInt16LE(entries.length, 10); end.writeUInt32LE(centralBytes.length, 12)
  end.writeUInt32LE(offset, 16)
  return Buffer.concat([...local, centralBytes, end])
}

function tar(entries) {
  const blocks = []
  for (const entry of entries) {
    if (Buffer.byteLength(entry.name) > 100) throw new Error('release archive path exceeds ustar name limit')
    const header = Buffer.alloc(512)
    header.write(entry.name, 0, 100, 'utf8')
    header.write((entry.executable ? '0000755' : '0000644') + '\0', 100, 8, 'ascii')
    header.write('0000000\0', 108, 8, 'ascii'); header.write('0000000\0', 116, 8, 'ascii')
    header.write(`${entry.bytes.length.toString(8).padStart(11, '0')}\0`, 124, 12, 'ascii')
    header.write('00000000000\0', 136, 12, 'ascii'); header.fill(0x20, 148, 156)
    header[156] = 0x30; header.write('ustar\0', 257, 6, 'ascii'); header.write('00', 263, 2, 'ascii')
    header.write('root', 265, 4, 'ascii'); header.write('root', 297, 4, 'ascii')
    const checksum = [...header].reduce((sum, byte) => sum + byte, 0)
    header.write(`${checksum.toString(8).padStart(6, '0')}\0 `, 148, 8, 'ascii')
    blocks.push(header, entry.bytes, Buffer.alloc((512 - (entry.bytes.length % 512)) % 512))
  }
  blocks.push(Buffer.alloc(1024))
  return Buffer.concat(blocks)
}

let bytes
if (platform === 'windows-x64') {
  const executable = join(releaseRoot, 'origami2-desktop.exe')
  const entries = [
    { name: basename(executable), bytes: readFileSync(executable), executable: true },
    ...filesBelow(join(releaseRoot, 'fonts'), 'fonts/'),
    ...filesBelow(join(releaseRoot, 'licenses'), 'licenses/'),
  ].sort((left, right) => left.name.localeCompare(right.name))
  bytes = zip(entries)
} else {
  const entries = filesBelow(join(releaseRoot, 'bundle', 'macos', 'ORIGAMI2.app'), 'ORIGAMI2.app/')
    .sort((left, right) => left.name.localeCompare(right.name))
  bytes = gzipSync(tar(entries), { level: 9, mtime: 0 })
}
writeFileSync(output, bytes)
console.log(`created reproducible ${platform} archive ${createHash('sha256').update(bytes).digest('hex')}`)
