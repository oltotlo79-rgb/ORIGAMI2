import { chromium } from 'playwright'
import { spawn } from 'node:child_process'

const serverOutputLimit = 16_000
const maximumBrowserDiagnostics = 50
const maximumDiagnosticLength = 2_000

export async function runBrowserE2E({
  name,
  port,
  harnessPath,
  readyButtonName,
}, assertions) {
  const origin = `http://127.0.0.1:${port}`
  const server = spawn(
    process.execPath,
    [
      './node_modules/vite/bin/vite.js',
      '--host',
      '127.0.0.1',
      '--port',
      String(port),
      '--strictPort',
    ],
    { cwd: process.cwd(), stdio: ['ignore', 'pipe', 'pipe'] },
  )
  let serverOutput = ''
  let omittedServerOutputCharacters = 0
  const appendServerOutput = (chunk) => {
    const combined = serverOutput + chunk.toString()
    if (combined.length > serverOutputLimit) {
      omittedServerOutputCharacters += combined.length - serverOutputLimit
      serverOutput = combined.slice(-serverOutputLimit)
    } else {
      serverOutput = combined
    }
  }
  server.stdout.on('data', appendServerOutput)
  server.stderr.on('data', appendServerOutput)
  const serverClosed = new Promise((resolve) => server.once('close', resolve))

  let browser
  let page
  let stoppingServer = false
  let cleaningUp = false
  let failure
  const cleanupErrors = []
  const browserDiagnostics = []
  const browserErrors = []
  const recordBrowserDiagnostic = (kind, value) => {
    browserDiagnostics.push(`${kind}: ${String(value).slice(0, maximumDiagnosticLength)}`)
    if (browserDiagnostics.length > maximumBrowserDiagnostics) browserDiagnostics.shift()
  }
  const recordBrowserError = (kind, value) => {
    const diagnostic = `${kind}: ${String(value).slice(0, maximumDiagnosticLength)}`
    browserErrors.push(diagnostic)
    if (browserErrors.length > maximumBrowserDiagnostics) browserErrors.shift()
    recordBrowserDiagnostic(kind, value)
  }
  const unexpectedServerStop = new Promise((_, reject) => {
    server.once('error', (error) => {
      if (!stoppingServer) reject(new Error(`Vite failed to start: ${error.message}`))
    })
    server.once('exit', (code, signal) => {
      if (!stoppingServer) {
        reject(new Error(
          `Vite exited unexpectedly (code=${code ?? 'none'}, signal=${signal ?? 'none'})`,
        ))
      }
    })
  })

  try {
    await runWithGuards(async () => {
      await waitForServer(server, origin)
      browser = await chromium.launch({ headless: true })
      browser.on('disconnected', () => {
        if (!cleaningUp) {
          recordBrowserError('browser', 'Chromium disconnected before cleanup')
        }
      })
      page = await browser.newPage()
      page.setDefaultTimeout(30_000)
      page.setDefaultNavigationTimeout(45_000)
      page.on('console', (message) => {
        if (message.type() === 'error') recordBrowserError('console', message.text())
      })
      page.on('pageerror', (error) => {
        recordBrowserError('pageerror', error.stack ?? error.message)
      })
      page.on('requestfailed', (request) => {
        recordBrowserDiagnostic(
          'requestfailed',
          `${request.method()} ${request.url()} ${request.failure()?.errorText ?? 'unknown error'}`,
        )
      })
      page.on('crash', () => recordBrowserError('page', 'Chromium page crashed'))
      await page.goto(`${origin}${harnessPath}`, {
        waitUntil: 'domcontentloaded',
        timeout: 45_000,
      })
      await page.getByRole('button', { name: readyButtonName }).waitFor()
      await assertions(page)
      if (browserErrors.length) {
        throw new Error(`browser runtime errors: ${browserErrors.join(' | ')}`)
      }
    }, unexpectedServerStop, name)
  } catch (error) {
    failure = error
  } finally {
    cleaningUp = true
    try {
      await browser?.close()
    } catch (error) {
      cleanupErrors.push(`Chromium close failed: ${errorMessage(error)}`)
    }
    stoppingServer = true
    try {
      await stopServer(server, serverClosed)
    } catch (error) {
      cleanupErrors.push(`Vite cleanup failed: ${errorMessage(error)}`)
    }
  }

  if (failure || cleanupErrors.length) {
    const serverOutputPrefix = omittedServerOutputCharacters
      ? `[${omittedServerOutputCharacters} earlier characters omitted]\n`
      : ''
    throw new Error([
      failure ? errorMessage(failure) : 'browser E2E cleanup failed',
      `page: ${page?.url() ?? 'not created'}`,
      `browser diagnostics:\n${browserDiagnostics.join('\n') || '(none)'}`,
      `Vite output:\n${serverOutputPrefix}${serverOutput || '(none)'}`,
      cleanupErrors.length ? `cleanup diagnostics:\n${cleanupErrors.join('\n')}` : '',
    ].filter(Boolean).join('\n\n'))
  }
}

async function waitForServer(server, origin) {
  const deadline = Date.now() + 30_000
  let lastProbeError = 'no HTTP response'
  while (Date.now() < deadline) {
    if (server.exitCode !== null || server.signalCode !== null) {
      throw new Error(
        `Vite exited before readiness (code=${server.exitCode ?? 'none'}, `
        + `signal=${server.signalCode ?? 'none'})`,
      )
    }
    try {
      const response = await fetch(origin, { signal: AbortSignal.timeout(1_000) })
      if (response.ok) return
      lastProbeError = `HTTP ${response.status}`
    } catch (error) {
      lastProbeError = errorMessage(error)
      // The explicit startup deadline owns retry timing and diagnostics.
    }
    await delay(100)
  }
  throw new Error(`Vite did not become ready within 30 seconds; last probe: ${lastProbeError}`)
}

async function runWithGuards(operation, unexpectedServerStop, name) {
  let timeout
  const overallDeadline = new Promise((_, reject) => {
    timeout = setTimeout(
      () => reject(new Error(`${name} exceeded its 180 second deadline`)),
      180_000,
    )
  })
  try {
    await Promise.race([operation(), unexpectedServerStop, overallDeadline])
  } finally {
    clearTimeout(timeout)
  }
}

async function stopServer(server, serverClosed) {
  if (server.exitCode !== null || server.signalCode !== null) {
    if (!await resolvesWithin(serverClosed, 5_000)) {
      throw new Error('Vite exited but its output streams did not close')
    }
    return
  }
  server.kill('SIGTERM')
  if (await resolvesWithin(serverClosed, 5_000)) return
  server.kill('SIGKILL')
  if (!await resolvesWithin(serverClosed, 5_000)) {
    throw new Error('Vite did not exit after SIGTERM and SIGKILL')
  }
}

async function resolvesWithin(promise, milliseconds) {
  let timeout
  try {
    return await Promise.race([
      promise.then(() => true),
      new Promise((resolve) => { timeout = setTimeout(() => resolve(false), milliseconds) }),
    ])
  } finally {
    clearTimeout(timeout)
  }
}

function errorMessage(error) {
  return error instanceof Error ? error.stack ?? error.message : String(error)
}

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds))
}
