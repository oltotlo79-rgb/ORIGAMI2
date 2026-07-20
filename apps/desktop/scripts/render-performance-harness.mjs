const edgeCanvas = document.querySelector('#edges')
const triangleCanvas = document.querySelector('#triangles')
const context = edgeCanvas.getContext('2d', { alpha: false })
const gl = triangleCanvas.getContext('webgl', { alpha: false, antialias: false })
if (!context || !gl) throw new Error('required software Canvas/WebGL context is unavailable')

const vertexShader = gl.createShader(gl.VERTEX_SHADER)
gl.shaderSource(vertexShader, 'attribute vec2 p; void main(){gl_Position=vec4(p,0.0,1.0);}')
gl.compileShader(vertexShader)
const fragmentShader = gl.createShader(gl.FRAGMENT_SHADER)
gl.shaderSource(fragmentShader, 'precision mediump float; void main(){gl_FragColor=vec4(0.15,0.55,0.85,1.0);}')
gl.compileShader(fragmentShader)
const program = gl.createProgram(); gl.attachShader(program, vertexShader); gl.attachShader(program, fragmentShader); gl.linkProgram(program); gl.useProgram(program)
const vertices = new Float32Array(10_000 * 6)
for (let index = 0; index < 10_000; index += 1) {
  const x = ((index % 100) / 50) - 1; const y = ((Math.floor(index / 100)) / 50) - 1; const offset = index * 6
  vertices.set([x, y, x + 0.015, y, x, y + 0.015], offset)
}
const buffer = gl.createBuffer(); gl.bindBuffer(gl.ARRAY_BUFFER, buffer); gl.bufferData(gl.ARRAY_BUFFER, vertices, gl.STATIC_DRAW)
const position = gl.getAttribLocation(program, 'p'); gl.enableVertexAttribArray(position); gl.vertexAttribPointer(position, 2, gl.FLOAT, false, 0, 0)

function draw() {
  context.fillStyle = '#fff'; context.fillRect(0, 0, 800, 600); context.beginPath()
  for (let index = 0; index < 10_000; index += 1) { const x = index % 200 * 4; const y = Math.floor(index / 200) * 12; context.moveTo(x, y); context.lineTo(x + 3, y + 8) }
  context.strokeStyle = '#222'; context.stroke()
  gl.clearColor(1, 1, 1, 1); gl.clear(gl.COLOR_BUFFER_BIT); gl.drawArrays(gl.TRIANGLES, 0, 30_000); gl.finish()
}

window.runOrigami2RenderBenchmark = async () => {
  for (let index = 0; index < 10; index += 1) { draw(); await frame() }
  const durations = []
  for (let index = 0; index < 30; index += 1) { const start = performance.now(); draw(); durations.push(performance.now() - start); await frame() }
  const sorted = [...durations].sort((a, b) => a - b)
  return { schema: 'origami2.software-render-benchmark.v1', edgeCount: 10_000, triangleCount: 10_000, warmupFrames: 10, measuredFrames: 30, medianFrameWorkMs: sorted[14], p95FrameWorkMs: sorted[28], worstFrameWorkMs: sorted[29], renderer: gl.getParameter(gl.RENDERER) }
}
const frame = () => new Promise((resolve) => requestAnimationFrame(resolve))
