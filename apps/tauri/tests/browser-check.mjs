// Run with PLAYWRIGHT_MODULE=/absolute/path/to/playwright/index.mjs node tests/browser-check.mjs.
import assert from 'node:assert/strict';
import { mkdir } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';
const modulePath = process.env.PLAYWRIGHT_MODULE;
if (!modulePath) throw new Error('Set PLAYWRIGHT_MODULE to an installed playwright/index.mjs');
const { chromium } = await import(pathToFileURL(modulePath));
const browser = await chromium.launch({ executablePath: process.env.CHROMIUM ?? '/usr/bin/chromium', headless: true });
const results = [], errors = [];
const context = await browser.newContext({ viewport: { width: 1100, height: 800 } });
const page = await context.newPage();
page.on('pageerror', e => errors.push(e.message));
const snapshot = () => page.evaluate(async () => {
  const { reader } = await import('/app.js');
  const s = reader.snapshot;
  return { at: s.at, mode: s.mode, page: s.page, pages: s.pages, speed: s.speed, bookId: s.book.book_id };
});
async function check(name, body) {
  try { await body(); results.push({ name, passed: true }); }
  catch (error) { results.push({ name, passed: false, error: error.message }); }
}
try {
  await page.goto('http://127.0.0.1:18772');
  await page.waitForFunction(() => document.querySelectorAll('#book [data-piece]').length > 20);
  await check('keyboard conventional pagination', async () => {
    const before = await snapshot(); assert.ok(before.pages > 1);
    await page.keyboard.press('ArrowRight'); assert.equal((await snapshot()).page, 1);
    await page.keyboard.press('ArrowLeft'); assert.equal((await snapshot()).page, 0);
  });
  let selected;
  await check('real mouse passage selection maps to source piece', async () => {
    const boxes = await page.locator('#book [data-piece]').evaluateAll(elements => elements.slice(5, 9).map(el => {
      const r = el.getBoundingClientRect(); return { x:r.x, y:r.y, width:r.width, height:r.height, piece:Number(el.dataset.piece) };
    }));
    const first = boxes[0], last = boxes[3];
    await page.mouse.move(first.x + 2, first.y + first.height / 2);
    await page.mouse.down();
    await page.mouse.move(last.x + last.width - 2, last.y + last.height / 2, { steps: 15 });
    await page.mouse.up();
    assert.ok(await page.evaluate(() => window.getSelection().toString().length > 0));
    selected = (await snapshot()).at;
    assert.equal(selected, first.piece);
  });
  await check('Space plays selected passage then pauses; Escape returns last piece', async () => {
    await page.keyboard.press('Space');
    const playing = await snapshot(); assert.equal(playing.mode, 'playing'); assert.equal(playing.at, selected);
    await page.waitForTimeout(400);
    await page.keyboard.press('Space');
    const paused = await snapshot(); assert.equal(paused.mode, 'paused');
    await page.waitForTimeout(250); assert.equal((await snapshot()).at, paused.at);
    await page.keyboard.press('Escape');
    const returned = await snapshot(); assert.equal(returned.mode, 'reading'); assert.equal(returned.at, paused.at);
  });
  await check('font reflow preserves source piece', async () => {
    await page.locator('#settings-button').click();
    const before = await snapshot();
    await page.locator('#font-size').focus();
    await page.keyboard.press('End');
    await page.waitForTimeout(100);
    const after = await snapshot(); assert.equal(after.at, before.at);
    assert.equal(await page.locator('#font-size').inputValue(), '32');
    assert.ok(after.pages >= before.pages);
  });
  await check('reload resumes stopped at same source and speed', async () => {
    await page.locator('#speed').fill('425'); await page.locator('#speed').press('Tab');
    const before = await snapshot(); assert.equal(before.speed, 425);
    await page.evaluate(async () => (await import('/app.js')).reader.flush());
    await page.reload(); await page.waitForFunction(() => document.querySelectorAll('#book [data-piece]').length > 20);
    const after = await snapshot(); assert.equal(after.mode, 'reading'); assert.equal(after.at, before.at); assert.equal(after.speed, 425);
  });
  await check('invalid file import retains current book', async () => {
    const before = await snapshot();
    await page.locator('#open').setInputFiles({ name:'invalid.json', mimeType:'application/json', buffer:Buffer.from('{"schema_version":999}') });
    await page.waitForFunction(() => document.querySelector('#notice').textContent.includes('Could not open'));
    const after = await snapshot(); assert.equal(after.bookId, before.bookId); assert.equal(after.at, before.at);
  });
  await mkdir(new URL('../.evidence/', import.meta.url), { recursive:true });
  await page.screenshot({ path:new URL('../.evidence/chromium-synthetic.png', import.meta.url).pathname });
  await check('built-in DOM tests', async () => {
    const clean = await browser.newContext({ viewport:{ width:1100, height:800 } });
    const tab = await clean.newPage();
    await tab.goto('http://127.0.0.1:18772/?self-test');
    await tab.waitForFunction(() => document.body.dataset.testResult, { timeout:30000 });
    const result = await tab.evaluate(() => JSON.parse(document.body.dataset.testResult));
    results.push({ name:'built-in DOM report', result });
    assert.equal(result.passed, true);
    await clean.close();
  });
  assert.deepEqual(errors, []);
} finally {
  console.log(JSON.stringify({ browser:await browser.version(), results, errors }, null, 2));
  await browser.close();
}
if (results.some(r => r.passed === false)) process.exitCode = 1;
