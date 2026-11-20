import { spawn } from 'node:child_process';
import fs from 'node:fs';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';

const ROOT = path.resolve('C:/Users/Ashutosh/Cyberground/spice');
const STUDIO_DIR = path.join(ROOT, 'demo_studio');
const FRAMES_DIR = path.join(STUDIO_DIR, 'frames');
const argOut = process.argv[2] ? path.resolve(ROOT, process.argv[2]) : path.join(ROOT, 'demo', 'AegisMesh_Product_Demo.mp4');
const argCrf = process.argv.includes('--crf') ? process.argv[process.argv.indexOf('--crf') + 1] : '17';
const OUT_MP4 = argOut;
const MASTER_WAV = path.join(STUDIO_DIR, 'audio', 'master_audio.wav');
const TIMELINE_PATH = path.join(STUDIO_DIR, 'timeline.json');
const CHROME_PATH = 'C:/Program Files/Google/Chrome/Application/chrome.exe';

const WORKERS = 6;
const BASE_CDP_PORT = 9241;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.json': 'application/json',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.wav': 'audio/wav',
};

function startStaticServer() {
  const srv = http.createServer((req, res) => {
    const cleanUrl = (req.url || '/').split('?')[0];
    const rel = cleanUrl === '/' ? '/stage.html' : cleanUrl;
    const filePath = path.join(STUDIO_DIR, rel);
    if (!filePath.startsWith(STUDIO_DIR) || !fs.existsSync(filePath)) {
      res.writeHead(404);
      res.end('Not Found');
      return;
    }
    const ext = path.extname(filePath);
    res.writeHead(200, { 'Content-Type': MIME[ext] || 'application/octet-stream' });
    fs.createReadStream(filePath).pipe(res);
  });
  return new Promise((resolve) => srv.listen(0, '127.0.0.1', () => resolve({ srv, port: srv.address().port })));
}

async function waitTargets(port) {
  for (let i = 0; i < 60; i++) {
    try {
      const r = await fetch(`http://127.0.0.1:${port}/json/list`);
      if (r.ok) {
        const list = await r.json();
        const page = list.find((t) => t.type === 'page');
        if (page?.webSocketDebuggerUrl) return page.webSocketDebuggerUrl;
      }
    } catch {
      // retry
    }
    await sleep(200);
  }
  throw new Error(`Chrome CDP port ${port} timed out`);
}

class FastCdp {
  constructor(wsUrl) {
    this.ws = new WebSocket(wsUrl);
    this.id = 0;
    this.pending = new Map();
    this.ready = new Promise((res, rej) => {
      this.ws.addEventListener('open', res);
      this.ws.addEventListener('error', rej);
    });
    this.ws.addEventListener('message', (ev) => {
      const m = JSON.parse(ev.data.toString());
      if (m.id && this.pending.has(m.id)) {
        const { res, rej } = this.pending.get(m.id);
        this.pending.delete(m.id);
        if (m.error) rej(new Error(m.error.message));
        else res(m.result);
      }
    });
  }
  async send(method, params = {}) {
    await this.ready;
    const id = ++this.id;
    return new Promise((res, rej) => {
      this.pending.set(id, { res, rej });
      this.ws.send(JSON.stringify({ id, method, params }));
    });
  }
  close() {
    try {
      this.ws.close();
    } catch {
      // ignore
    }
  }
}

async function renderShard(workerIdx, startFrame, endFrame, fps, stageUrl, tempRoot) {
  const port = BASE_CDP_PORT + workerIdx;
  const prof = path.join(tempRoot, `prof_${workerIdx}`);
  const proc = spawn(
    CHROME_PATH,
    [
      '--headless=new',
      `--remote-debugging-port=${port}`,
      `--user-data-dir=${prof}`,
      '--window-size=1920,1080',
      '--no-first-run',
      '--no-default-browser-check',
      '--disable-background-networking',
      '--hide-scrollbars',
      'about:blank',
    ],
    { stdio: 'ignore' },
  );

  let cdp = null;
  try {
    const wsUrl = await waitTargets(port);
    cdp = new FastCdp(wsUrl);
    await cdp.send('Page.enable');
    await cdp.send('Runtime.enable');
    await cdp.send('Emulation.setDeviceMetricsOverride', {
      width: 1920,
      height: 1080,
      deviceScaleFactor: 1,
      mobile: false,
    });
    await cdp.send('Page.navigate', { url: stageUrl });
    await sleep(1400);
    await cdp.send('Runtime.evaluate', {
      expression: `(async () => { await document.fonts.ready; await window.renderAt(0); })()`,
      awaitPromise: true,
    });
    await sleep(400);

    for (let f = startFrame; f < endFrame; f++) {
      const t = (f / fps).toFixed(4);
      await cdp.send('Runtime.evaluate', {
        expression: `window.renderAt(${t})`,
        awaitPromise: true,
      });
      const { data } = await cdp.send('Page.captureScreenshot', {
        format: 'jpeg',
        quality: 94,
        fromSurface: true,
      });
      const name = `frame_${String(f).padStart(6, '0')}.jpg`;
      fs.writeFileSync(path.join(FRAMES_DIR, name), Buffer.from(data, 'base64'));
      if ((f - startFrame) % 120 === 0) {
        console.log(`[W${workerIdx}] rendered frame ${f}/${endFrame}`);
      }
    }
  } finally {
    if (cdp) cdp.close();
    if (!proc.killed) proc.kill();
  }
}

async function main() {
  const tl = JSON.parse(fs.readFileSync(TIMELINE_PATH, 'utf8'));
  const fps = tl.fps || 60;
  const totalFrames = Math.ceil(tl.duration * fps);
  console.log(`[RENDER] ${totalFrames} frames @ ${fps}fps (${tl.duration.toFixed(2)}s) across ${WORKERS} workers`);

  fs.rmSync(FRAMES_DIR, { recursive: true, force: true });
  fs.mkdirSync(FRAMES_DIR, { recursive: true });
  fs.mkdirSync(path.dirname(OUT_MP4), { recursive: true });

  const { srv, port } = await startStaticServer();
  const stageUrl = `http://127.0.0.1:${port}/stage.html`;
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'aegismesh_vid_'));

  try {
    const shardSize = Math.ceil(totalFrames / WORKERS);
    const jobs = [];
    for (let w = 0; w < WORKERS; w++) {
      const sf = w * shardSize;
      const ef = Math.min(totalFrames, sf + shardSize);
      if (sf < ef) jobs.push(renderShard(w, sf, ef, fps, stageUrl, tempRoot));
    }
    await Promise.all(jobs);

    console.log('[ENCODE] Encoding 1920x1080 60fps H.264 + 48kHz AAC...');
    await new Promise((resolve, reject) => {
      const ff = spawn(
        'ffmpeg',
        [
          '-y',
          '-framerate', String(fps),
          '-i', path.join(FRAMES_DIR, 'frame_%06d.jpg'),
          '-i', MASTER_WAV,
          '-c:v', 'libx264',
          '-preset', 'medium',
          '-crf', String(argCrf),
          '-pix_fmt', 'yuv420p',
          '-c:a', 'aac',
          '-b:a', '192k',
          '-shortest',
          '-movflags', '+faststart',
          OUT_MP4,
        ],
        { stdio: 'inherit' },
      );
      ff.on('exit', (code) => (code === 0 ? resolve() : reject(new Error(`ffmpeg exited ${code}`))));
    });
    console.log(`[DONE] Rendered ${OUT_MP4}`);
  } finally {
    srv.close();
    await sleep(300);
    fs.rmSync(tempRoot, { recursive: true, force: true });
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
