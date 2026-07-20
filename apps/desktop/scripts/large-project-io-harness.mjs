import { createBrowserBenchmarkPattern } from '../src/lib/coreClient.ts'

const encode = new TextEncoder()
const frame = () => new Promise((resolve) => requestAnimationFrame(resolve))
const measure = async (operation, count = 20) => {
  const samples = []
  for (let index = 0; index < count; index += 1) {
    const start = performance.now(); await operation(index); samples.push(performance.now() - start); await frame()
  }
  samples.sort((a, b) => a - b)
  return { medianMs: samples[Math.floor(count / 2) - 1], p95Ms: samples[Math.ceil(count * 0.95) - 1], worstMs: samples[count - 1] }
}

window.runOrigami2LargeProjectIoBenchmark = async () => {
  const pattern = createBrowserBenchmarkPattern(10_000)
  const document = { schema: 'origami2.browser-native-boundary-fixture.v1', revision: 0, pattern }
  const bytes = JSON.stringify(document)
  const byteLength = encode.encode(bytes).byteLength
  if (pattern.edge_count !== 10_000 || byteLength > 16 * 1024 * 1024) throw new Error('large project fixture is outside bounds')
  for (let index = 0; index < 5; index += 1) JSON.parse(bytes)
  const heapBefore = performance.memory?.usedJSHeapSize ?? null
  const save = await measure(() => { const saved = JSON.stringify(document); if (saved.length !== bytes.length) throw new Error('save bytes drifted') })
  const open = await measure(() => { const opened = JSON.parse(bytes); if (opened.pattern.edge_count !== 10_000) throw new Error('open edge count drifted') })
  const history = []
  const undoRedo = await measure((index) => {
    const edge = pattern.edges[index % pattern.edges.length]; const before = edge.kind; const after = before === 'mountain' ? 'valley' : 'mountain'
    history.push({ edgeId: edge.id, before, after }); edge.kind = after; edge.kind = history.pop().before
    if (edge.kind !== before) throw new Error('undo/redo round trip drifted')
  })
  const heapAfter = performance.memory?.usedJSHeapSize ?? null
  return { schema: 'origami2.large-project-io-benchmark.v1', edgeCount: pattern.edge_count, vertexCount: pattern.vertex_count, serializedBytes: byteLength, samplesPerOperation: 20, save, open, undoRedo, heapGrowthBytes: heapBefore === null || heapAfter === null ? null : Math.max(0, heapAfter - heapBefore), boundary: 'browser-mock-native-ipc' }
}
