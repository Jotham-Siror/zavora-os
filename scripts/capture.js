// Captures the "build a pitch deck → combine" beat from field.html as PNG frames.
// Uses the system Chrome via puppeteer-core. Frames -> ffmpeg -> gif/mp4.
const puppeteer = require('puppeteer-core');
const path = require('path');
const fs = require('fs');

const CHROME = '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const URL = 'file://' + path.resolve(__dirname, '../web/index.html') + '?demo=1';
const OUT = path.resolve(__dirname, '../demo/frames');
const W = 1280, H = 800, FPS = 20, SECONDS = 14;

(async () => {
  fs.rmSync(OUT, { recursive: true, force: true });
  fs.mkdirSync(OUT, { recursive: true });
  const browser = await puppeteer.launch({ executablePath: CHROME, headless: 'new',
    args: [`--window-size=${W},${H}`, '--hide-scrollbars', '--force-device-scale-factor=1'] });
  const page = await browser.newPage();
  await page.setViewport({ width: W, height: H, deviceScaleFactor: 1 });
  await page.goto(URL, { waitUntil: 'networkidle0' });

  // capture continuously across the whole opening arc
  const total = FPS * SECONDS, interval = 1000 / FPS;
  let i = 0;
  const grab = async () => {
    await page.screenshot({ path: path.join(OUT, `f${String(i).padStart(4,'0')}.png`) });
    i++;
  };

  // beat 1: greeting on screen
  await page.waitForFunction('typeof launch === "function"', { timeout: 5000 });
  await new Promise(r => setTimeout(r, 600));
  for (let k = 0; k < FPS * 2 && i < total; k++) { await grab(); await new Promise(r => setTimeout(r, interval)); }

  // beat 2: press "Start my day" → a fixed scenario (morning) blooms
  await page.evaluate(() => {
    clearTimeout(window.idleTimer);
    document.getElementById('greet').classList.remove('show');
    launch('Start my day');
  });

  // beat 3: capture the bloom → resolve → suggestion appearing
  while (i < total) { await grab(); await new Promise(r => setTimeout(r, interval)); }

  await browser.close();
  console.log('captured', i, 'frames to', OUT);
})();
