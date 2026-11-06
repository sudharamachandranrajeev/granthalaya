// Takes one screenshot in headless Chromium over the DevTools protocol, in real
// time (virtual time stalls on the reader's page animations). Prints the page's
// console errors so a broken screen is noticed.
//   node scripts/shot.mjs <url> <out.png> <width> <height> [waitMs]
import { spawn } from 'node:child_process'
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

const [url, out, w = '1280', h = '800', waitMs = '5000'] = process.argv.slice(2)
const profile = mkdtempSync(join(tmpdir(), 'shot-'))
const chrome = spawn(process.env.CHROMIUM || 'chromium', ['--headless=new', '--disable-gpu', '--no-sandbox', '--hide-scrollbars', `--user-data-dir=${profile}`,
  `--window-size=${w},${h}`, '--remote-debugging-port=0', 'about:blank'], { stdio: ['ignore', 'ignore', 'pipe'] })
const wsUrl = await new Promise((res) => chrome.stderr.on('data', (d) => { const m = /ws:\/\/\S+/.exec(String(d)); if (m) res(m[0]) }))
const base = wsUrl.replace(/^ws/, 'http').replace(/\/devtools.*/, '')
const target = (await (await fetch(`${base}/json/list`)).json()).find((t) => t.type === 'page')
const ws = new WebSocket(target.webSocketDebuggerUrl)
await new Promise((r) => ws.addEventListener('open', r))
let n = 0
const pending = new Map()
ws.addEventListener('message', (e) => {
  const m = JSON.parse(e.data)
  if (m.id && pending.has(m.id)) { pending.get(m.id)(m.result); pending.delete(m.id) }
  if (m.method === 'Runtime.consoleAPICalled' && ['error', 'warning'].includes(m.params.type)) console.log('console.' + m.params.type, m.params.args.map((a) => a.value ?? a.description).join(' '))
  if (m.method === 'Runtime.exceptionThrown') console.log('EXC', m.params.exceptionDetails.exception?.description ?? m.params.exceptionDetails.text)
})
const send = (method, params = {}) => new Promise((r) => { const id = ++n; pending.set(id, r); ws.send(JSON.stringify({ id, method, params })) })
await send('Runtime.enable')
await send('Page.enable')
await send('Emulation.setDeviceMetricsOverride', { width: +w, height: +h, deviceScaleFactor: 1, mobile: +w < 600 })
await send('Page.navigate', { url })
await new Promise((r) => setTimeout(r, +waitMs))
const shot = await Promise.race([send('Page.captureScreenshot', out.endsWith('.jpg') ? { format: 'jpeg', quality: 82 } : { format: 'png' }), new Promise((r) => setTimeout(() => r(null), 8000))])
if (shot) writeFileSync(out, Buffer.from(shot.data, 'base64')); else console.log('screenshot timed out (page hung)')
ws.close(); chrome.kill(); rmSync(profile, { recursive: true, force: true })
