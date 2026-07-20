const MAX_ARCHIVE_ENTRIES = 100_000

export function validateReleaseArchiveEntries(platform, entries) {
  if (!['windows-x64', 'macos-arm64'].includes(platform)) {
    throw new Error('unsupported release archive platform')
  }
  if (!Array.isArray(entries) || entries.length === 0 || entries.length > MAX_ARCHIVE_ENTRIES) {
    throw new Error('release archive entry count is invalid')
  }

  const expectedRoot = platform === 'windows-x64' ? null : 'ORIGAMI2.app'
  let portableExecutableFound = false
  const seen = new Set()
  for (const entry of entries) {
    if (
      typeof entry !== 'string'
      || entry.length === 0
      || entry.includes('\0')
      || entry.includes('\\')
      || entry.startsWith('/')
      || /^[A-Za-z]:/u.test(entry)
    ) {
      throw new Error('release archive contains an unsafe path')
    }
    const segments = entry.replace(/\/$/u, '').split('/')
    if (
      segments.some((segment) => segment === '' || segment === '.' || segment === '..')
    ) {
      throw new Error('release archive contains a traversal path')
    }
    const canonicalEntry = segments.join('/')
    if (seen.has(canonicalEntry)) {
      throw new Error('release archive contains duplicate entries')
    }
    seen.add(canonicalEntry)
    if (expectedRoot !== null && segments[0] !== expectedRoot) {
      throw new Error('macOS release archive has an unexpected root')
    }
    if (
      platform === 'windows-x64'
      && !['origami2-desktop.exe', 'fonts', 'licenses'].includes(segments[0])
    ) {
      throw new Error('portable release archive has an unexpected root')
    }
    if (platform === 'windows-x64' && entry === 'origami2-desktop.exe') {
      portableExecutableFound = true
    }
  }
  if (platform === 'windows-x64' && !portableExecutableFound) {
    throw new Error('portable archive executable contract failed')
  }
  return true
}
