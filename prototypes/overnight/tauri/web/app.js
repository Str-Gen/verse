import { Comfort } from './comfort.js';
import { VocabularyUI } from './vocabulary.js';
import { startTheme } from './theme.js';
import { validateBook, sourceIndex, runSegments, chapterAt, position, restore, localImage } from './model.js';
const $ = id => document.getElementById(id);
const native = window.__TAURI__?.core;
const storageKey = 'overnight-tauri-browser-v1';
let book, saved = { books: Object.create(null) }, at = 0, page = 0, pages = 1, pageWidth = 1;
let mode = 'reading', timer, nextChapter = null, guarded = false, recoveryAt = null;
let startup = null, notePlayback = null, policy = null;
let windowInfo = null, navigating = false, navigationEpoch = 0, playbackEpoch = 0;
let imageEpoch = 0, imageCache = new Map(), imageBusy = false, imageAgain = false;
let pendingCheckpoint = null, writingCheckpoint = false, lastCheckpoint = 0;
let paceChanges = [], writeQueue = Promise.resolve(), sourceMap, pageForPiece = new Map();
const blocks = new Map();
let comfort = new Comfort(), preparing = false, exposures = [], exposure = null;
const vocabulary = new VocabularyUI(async request => {
  if (!native) throw new Error('Vocabulary is available in the native app.');
  if (request.action === 'capture') {
    finishExposure();
    request = {...request, exposures, now: performance.now(), tap_id: crypto.randomUUID()};
    if (mode === 'playing' && !preparing) beginExposure();
  }
  return native.invoke('vocabulary', {request});
}, () => { if (mode === 'playing') { stop(); mode = 'paused'; } }, speed, notice);
function beginExposure() { exposure = !notePlayback && kindAt(at) === 'word' ? {piece: at, start: performance.now()} : null; }
function finishExposure() {
  if (exposure) { const end = performance.now(); if (end > exposure.start) exposures.push({...exposure, end}); exposure = null; exposures = exposures.slice(-2000); }
}
function beginPlayback() {
  const delay = comfort.start(performance.now());
  if (!delay) { schedule(); return; }
  preparing = true; mode = 'playing'; $('playback').classList.add('preparing', 'running');
  timer = setTimeout(() => { preparing = false; $('playback').classList.remove('preparing'); schedule(); }, delay);
}
function notice(text = '') { $('notice').textContent = text; }
function preferences() { return Object.hasOwn(saved.books, book.book_id) ? saved.books[book.book_id] : {}; }
function speed() { return Math.max(60, Math.min(1200, Number($('speed').value) || 350)); }
function persist(force = false) {
  if (!book || guarded) return;
  saved.books[book.book_id] = { ...preferences(), ...position(book, at), speed: speed(), fontSize: Number($('font-size').value), feedback: $('feedback-enabled').checked, followNotes: $('follow-notes').checked, pendingChanges: paceChanges, pendingChapter: chapterAt(book, at)?.id ?? null, history: preferences().history ?? [] };
  if (!force && mode === 'playing' && performance.now() - lastCheckpoint < 2000) return;
  lastCheckpoint = performance.now();
  pendingCheckpoint = {state: JSON.parse(JSON.stringify(saved)), savedAt: (windowInfo?.start ?? 0) + at, bookId: book.book_id};
  if (writingCheckpoint) return;
  writingCheckpoint = true;
  writeQueue = (async () => {
    while (pendingCheckpoint) {
      const {state, savedAt, bookId} = pendingCheckpoint;
      pendingCheckpoint = null;
      try {
        if (native) {
          const record = await native.invoke('save_state', {state, at: savedAt, bookId});
          if (saved.books[bookId]?.revision === record.revision) saved.books[bookId].location = record.location;
        } else localStorage.setItem(storageKey, JSON.stringify({book, state}));
      } catch { notice('Could not save this session. Check scratch storage before closing.'); }
    }
  })().finally(() => { writingCheckpoint = false; });
}
function choose(index, { explicit = true } = {}) {
  if (!Number.isInteger(index) || index < 0 || index >= book.pieces.length) return;
  if (explicit) {
    guarded = false;
    if (chapterAt(book, at)?.id !== chapterAt(book, index)?.id) paceChanges = [];
  }
  at = index;
  if (native && kindAt(at) === 'image') void refreshImages();
  document.querySelectorAll('.selected').forEach(el => el.classList.remove('selected'));
  document.querySelectorAll(`[data-piece="${at}"]`).forEach(el => el.classList.add('selected'));
  persist();
}
function makeText(run) {
  const wrapper = document.createElement(run.note_id ? 'button' : run.strong ? 'strong' : 'span');
  if (run.note_id) {
    wrapper.className = 'note-link'; wrapper.title = 'Read note';
    wrapper.addEventListener('click', () => {
      const note = book.notes.find(n => n.id === run.note_id);
      showDetails('Note', note ? note.blocks.flatMap(b => b.runs.map(r => r.text)).join(' ') : 'This note is unavailable.');
    });
  }
  const parent = run.emphasis ? wrapper.appendChild(document.createElement('em')) : wrapper;
  for (const segment of runSegments(run, sourceMap)) {
    const el = document.createElement('span');
    el.textContent = segment.text;
    if (segment.piece !== null) el.dataset.piece = segment.piece;
    parent.append(el);
  }
  return wrapper;
}
function render() {
  const article = $('book'); article.replaceChildren(); blocks.clear(); sourceMap = sourceIndex(book);
  const firstPieces = new Map();
  book.pieces.forEach((piece, index) => { if (!firstPieces.has(piece.block_id)) firstPieces.set(piece.block_id, index); });
  for (const doc of book.documents) for (const block of doc.blocks) {
    blocks.set(block.id, block);
    if (block.kind === 'ornament') continue;
    const first = firstPieces.get(block.id) ?? -1;
    let element;
    if (block.kind === 'note') {
      element = document.createElement('p');
      const button = document.createElement('button'); button.textContent = 'Read note';
      button.addEventListener('click', () => showDetails('Note', block.runs.map(r => r.text).join('')));
      element.append(button);
    } else if (block.kind === 'image') {
      element = document.createElement('figure');
      if (native) { const img = document.createElement('img'); img.dataset.resource = block.resource ?? ''; img.alt = block.alt ?? 'Book image'; img.addEventListener('load', reflow); element.append(img); }
      const resource = native ? null : localImage(book.resources?.find(r => r.id === block.resource)?.data_url ?? block.resource);
      if (resource) { const img = document.createElement('img'); img.src = resource; img.alt = block.alt ?? 'Book image'; img.addEventListener('load', reflow); element.append(img); }
      if (!resource && !native) { const caption = document.createElement('figcaption'); caption.textContent = `${block.alt?.trim() || 'Image'} · Image unavailable.`; element.append(caption); }
    } else if (['heading', 'paragraph', 'list_item'].includes(block.kind)) {
      element = document.createElement(block.kind === 'heading' ? `h${[1,2,3].includes(block.level) ? block.level : 2}` : block.kind === 'list_item' ? 'li' : 'p');
      block.runs.forEach(run => element.append(makeText(run)));
    } else {
      element = document.createElement('p'); element.className = 'placeholder'; element.textContent = `Unsupported content: ${block.alt ?? block.kind}. Playback pauses here.`;
    }
    if (first >= 0 && !element.querySelector('[data-piece]')) element.dataset.piece = first;
    article.append(element);
  }
  if (native) void refreshImages();
  $('title').textContent = book.title;
  $('chapters').replaceChildren();
  const front = document.createElement('option'); front.value = '0'; front.textContent = 'Beginning'; $('chapters').append(front);
  for (const chapter of windowInfo?.chapters ?? book.chapters) {
    const option = document.createElement('option'); option.value = windowInfo ? chapter.at : book.pieces.findIndex(p => p.id === chapter.start_piece); option.textContent = chapter.title; $('chapters').append(option);
  }
}
function releaseImage(img) {
  img.removeAttribute('src');
  if (img.dataset.objectUrl) URL.revokeObjectURL(img.dataset.objectUrl);
}
function imageUrl(dataUrl) {
  const comma = dataUrl.indexOf(',');
  const type = dataUrl.slice(5, dataUrl.indexOf(';'));
  const binary = atob(dataUrl.slice(comma + 1));
  const bytes = Uint8Array.from(binary, c => c.charCodeAt(0));
  return URL.createObjectURL(new Blob([bytes], {type}));
}
function imageFailure(id) {
  for (const img of document.querySelectorAll('#book img[data-resource]')) {
    if (img.dataset.resource !== id || img.parentElement.querySelector('figcaption')) continue;
    const caption = document.createElement('figcaption');
    caption.textContent = `${img.alt || 'Image'} · Image unavailable.`;
    img.parentElement.append(caption);
  }
}
async function refreshImages() {
  if (!native || !book || !windowInfo) return;
  if (imageBusy) { imageAgain = true; return; }
  imageBusy = true;
  const epoch = ++imageEpoch, bookId = book.book_id;
  try {
    const visible = mode === 'reading' ? [...document.querySelectorAll('#book img[data-resource]')].filter(img => pageOfElement(img) === page).map(img => Number(img.closest('[data-piece]')?.dataset.piece)).filter(Number.isInteger) : [];
    const imageAt = visible.length ? visible[Math.floor(visible.length / 2)] : at;
    const resources = await native.invoke('image_resources', {at: windowInfo.start + imageAt, bookId});
    if (epoch !== imageEpoch || book.book_id !== bookId) return;
    const wanted = new Set(resources.map(r => r.id));
    for (const [id, img] of imageCache) if (!wanted.has(id)) { releaseImage(img); imageCache.delete(id); }
    for (const el of document.querySelectorAll('#book img[data-resource]')) {
      if (!wanted.has(el.dataset.resource)) el.removeAttribute('src');
    }
    await Promise.all(resources.map(async resource => {
      if (imageCache.has(resource.id)) return;
      const url = localImage(resource.data_url || resource.id); if (!url) { imageFailure(resource.id); return; }
      const img = new Image();
      if (url.startsWith('data:')) { img.dataset.objectUrl = imageUrl(url); img.src = img.dataset.objectUrl; }
      else img.src = url;
      try { await img.decode(); } catch { releaseImage(img); if (epoch === imageEpoch) imageFailure(resource.id); return; }
      if (epoch === imageEpoch && book.book_id === bookId) imageCache.set(resource.id, img);
      else releaseImage(img);
    }));
    if (epoch !== imageEpoch) return;
    for (const el of document.querySelectorAll('#book img[data-resource]')) {
      const cached = imageCache.get(el.dataset.resource);
      if (cached) { el.parentElement.querySelector('figcaption')?.remove(); if (el.src !== cached.src) el.src = cached.src; }
    }
  } catch (error) { if (epoch === imageEpoch) notice(`Image unavailable: ${error}`); }
  finally { imageBusy = false; if (imageAgain) { imageAgain = false; queueMicrotask(refreshImages); } }
}
function pageOfElement(el) {
  const rect = el.getClientRects()[0];
  if (!rect) return 0;
  return Math.max(0, Math.floor((rect.left - $('viewport').getBoundingClientRect().left + page * pageWidth + 1) / pageWidth));
}
function showPage(value) {
  page = Math.max(0, Math.min(pages - 1, value));
  $('book').style.transform = `translateX(${-page * pageWidth}px)`;
  $('page-label').textContent = `${page + 1} / ${pages}`;
  $('previous').disabled = page === 0 && !(windowInfo?.start > 0); $('next').disabled = page >= pages - 1 && !hasNextWindow();
}
function reflow() {
  if (!book) return;
  $('viewport').style.fontSize = `${$('font-size').value}px`;
  $('book').style.fontSize = `${$('font-size').value}px`;
  $('book').style.setProperty('--image-height', `${Math.max(1, $('viewport').clientHeight - Number($('font-size').value) * 3)}px`);
  const width = $('viewport').clientWidth;
  if (width <= 0) return;
  $('book').style.width = `${width}px`;
  $('book').style.columnWidth = `${width}px`;
  $('book').style.transform = 'none'; page = 0; pageWidth = width + 64;
  pages = Math.max(1, Math.ceil(($('book').scrollWidth + 64) / pageWidth));
  pageForPiece = new Map();
  document.querySelectorAll('#book [data-piece]').forEach(el => {
    const index = Number(el.dataset.piece);
    if (!pageForPiece.has(index)) pageForPiece.set(index, pageOfElement(el));
  });
  showPage(pageForPiece.get(at) ?? 0);
  choose(at, { explicit: false });
  if (native) void refreshImages();
}
function hasNextWindow() { return windowInfo && windowInfo.start + book.pieces.length < windowInfo.total; }
function globalChapter(index) {
  return windowInfo?.chapters.filter(c => c.at <= index).at(-1) ?? null;
}
async function moveWindow(destination) {
  if (!native || navigating) return false;
  navigating = true;
  const epoch = ++navigationEpoch;
  try {
    finishExposure();
    await writeQueue;
    const prepared = await native.invoke('load_window', {at: destination, bookId: book.book_id});
    if (epoch !== navigationEpoch) return false;
    const oldStart = windowInfo.start;
    policy = prepared; windowInfo = prepared.window;
    book = validateBook(prepared.book); at = destination - windowInfo.start;
    exposures = exposures.map(e => ({...e, piece: e.piece + oldStart - windowInfo.start})).filter(e => e.piece >= 0 && e.piece < book.pieces.length); exposure = null;
    render(); reflow();
    return true;
  } catch (error) {
    stop(); mode = 'paused'; notice(`Could not load this section: ${error}`);
    return false;
  } finally { navigating = false; }
}
async function turn(direction) {
  if (mode !== 'reading' || navigating) return;
  const target = Math.max(0, Math.min(pages - 1, page + direction));
  if (target === page) {
    if (direction > 0 && hasNextWindow()) await moveWindow(windowInfo.start + book.pieces.length);
    else if (direction < 0 && windowInfo?.start > 0) await moveWindow(windowInfo.start - 1);
    return;
  }
  showPage(target);
  const first = [...pageForPiece].find(([, p]) => p === page);
  if (first) choose(first[0]);
  if (native) void refreshImages();
}

function stop() { playbackEpoch++; finishExposure(); if (mode === 'playing') comfort.pause(performance.now()); preparing = false; clearTimeout(timer); timer = undefined; $('playback').classList.remove('running', 'preparing'); }
function returnToText() {
  stop(); comfort = new Comfort(policy?.comfort); notePlayback = null; mode = 'reading'; $('playback').hidden = true;
  if ($('boundary').open) $('boundary').close();
  reflow(); persist();
}
function kindAt(index) {
  if (policy) return policy.normal[index].kind;
  const piece = book.pieces[index], kind = piece.kind ?? blocks.get(piece.block_id)?.kind ?? 'word';
  return ['word','paragraph','heading','list_item','image','ornament','note','unsupported'].includes(kind) ? kind : 'unsupported';
}
function skipAt(index) { return kindAt(index) === 'ornament' || (kindAt(index) === 'note' && !$('follow-notes').checked); }
function enterNote(start = false) {
  const piece = book.pieces[at];
  if (kindAt(at) !== 'note' || !piece.note_id || !$('follow-notes').checked) return false;
  const note = book.notes.find(n => n.id === piece.note_id);
  if (!note?.pieces?.length) { stop(); mode = 'paused'; $('word').textContent = 'Note unavailable'; $('play-status').textContent = 'Space continues after the marker'; return true; }
  const actions = policy?.note_plans[note.id];
  const index = actions ? actions.findIndex(a => !['note', 'ornament'].includes(a.kind)) : 0;
  if (index < 0) return false;
  notePlayback = { note, index, actions }; displayNote();
  if (['image', 'unsupported'].includes(actions?.[index].kind)) { stop(); mode = 'paused'; }
  else if (start) beginPlayback();
  else schedule();
  return true;
}
function displayNote() { $('word').textContent = notePlayback.note.pieces[notePlayback.index].text; $('play-status').textContent = 'Note · Escape returns to its marker'; }
function displayCurrent() {
  const kind = kindAt(at), piece = book.pieces[at];
  $('word').textContent = ['image','unsupported'].includes(kind) ? blocks.get(piece.block_id)?.alt || piece.text || (kind === 'image' ? 'Image' : 'Unsupported content') : piece.text;
  $('play-status').textContent = ['image','unsupported'].includes(kind) ? 'Paused for content · Space to continue · Escape to view in context' : 'Paused · Space to continue · Escape to return';
  choose(at, { explicit: false });
}
function boundary(next) {
  stop(); mode = 'paused'; nextChapter = next;
  $('questions').hidden = !$('feedback-enabled').checked;
  $('skip-feedback').textContent = $('feedback-enabled').checked ? 'Skip and continue' : 'Continue';
  $('pace').value = 'comfortable'; $('understanding').value = 'easily'; $('suggestion').hidden = true; $('accept-speed').checked = false;
  $('boundary').showModal();
}
async function advance() {
  if (navigating) return;
  const epoch = playbackEpoch;
  finishExposure();
  clearTimeout(timer);
  if (notePlayback) {
    const action = notePlayback.actions?.[notePlayback.index];
    notePlayback.index = action ? action.next ?? notePlayback.note.pieces.length : notePlayback.index + 1;
    if (notePlayback.index < notePlayback.note.pieces.length) {
      displayNote();
      if (['image', 'unsupported'].includes(notePlayback.actions?.[notePlayback.index].kind)) { stop(); mode = 'paused'; }
      else schedule();
      return;
    }
    notePlayback = null;
  }
  const action = policy?.[$('follow-notes').checked ? 'notes' : 'normal'][at];
  let next = action ? action.next ?? book.pieces.length : at + 1;
  if (!action) while (next < book.pieces.length && skipAt(next)) next++;
  if (next >= book.pieces.length) {
    if (!hasNextWindow()) { boundary(next); return; }
    const destination = windowInfo.start + book.pieces.length;
    const outgoing = globalChapter(windowInfo.start + at), incoming = globalChapter(destination);
    if (outgoing && incoming?.id !== outgoing.id) { boundary(next); return; }
    if (!await moveWindow(destination) || epoch !== playbackEpoch) return;
    displayCurrent();
    if (enterNote()) return;
    if (['image','unsupported'].includes(kindAt(at))) { stop(); mode = 'paused'; return; }
    schedule(); return;
  }
  const currentChapter = chapterAt(book, at), incoming = chapterAt(book, next);
  if (action ? action.chapter_boundary : currentChapter && incoming?.id !== currentChapter.id) { boundary(next); return; }
  at = next; displayCurrent();
  if (enterNote()) return;
  if (['image','unsupported'].includes(kindAt(at))) { stop(); mode = 'paused'; return; }
  schedule();
}
function schedule() {
  mode = 'playing'; $('playback').classList.add('running');
  const piece = notePlayback ? notePlayback.note.pieces[notePlayback.index] : book.pieces[at];
  const timing = notePlayback ? notePlayback.actions?.[notePlayback.index] : policy?.normal[at];
  const multiplier = timing ? timing.multiplier : /[.!?][”'"’)]?$/.test(piece.text) ? 1.7 : /[,;:]$/.test(piece.text) ? 1.25 : 1;
  beginExposure();
  timer = setTimeout(advance, (60000 / speed() * multiplier + (timing ? timing.extra_ms : piece.paragraph_end ? 120 : 0)) / comfort.fraction(performance.now()));
}
async function play() {
  if (navigating) return;
  if (document.querySelector('dialog[open]')) return;
  if (mode === 'playing') { stop(); mode = 'paused'; $('play-status').textContent = 'Paused · Space to continue · Escape to return'; persist(); return; }
  if (mode === 'reading') {
    while (at < book.pieces.length && skipAt(at)) {
      const next = policy?.[$('follow-notes').checked ? 'notes' : 'normal'][at].next;
      if (policy && next === null || !policy && at === book.pieces.length - 1) {
        if (hasNextWindow()) {
          const epoch = playbackEpoch;
          if (await moveWindow(windowInfo.start + book.pieces.length) && epoch === playbackEpoch) await play();
        } else notice('End of book.');
        return;
      }
      at = policy ? next : at + 1;
    }
    $('playback').hidden = false; displayCurrent();
    if (enterNote(true)) return;
    if (['image','unsupported'].includes(kindAt(at))) { mode = 'paused'; return; }
    beginPlayback();
  } else if (['image','unsupported'].includes(notePlayback ? notePlayback.actions?.[notePlayback.index].kind : kindAt(at))) { advance(); if (mode === 'playing') { clearTimeout(timer); finishExposure(); beginPlayback(); } }
  else beginPlayback();
}
async function continueChapter(store) {
  const epoch = playbackEpoch;
  if (nextChapter === null) return;
  if (store) {
    const prior = speed();
    if ($('accept-speed').checked && $('pace').value !== 'comfortable') $('speed').value = Math.max(60, Math.min(1200, prior + ($('pace').value === 'fast' ? -25 : 25)));
    const history = preferences().history ?? [];
    history.push({ date: new Date().toISOString(), chapter: chapterAt(book, at)?.id, pace: $('pace').value, understanding: $('understanding').value, changes: paceChanges, speedBefore: prior, speedAfter: speed() });
    saved.books[book.book_id] = { ...preferences(), history };
  }
  paceChanges = []; const destination = nextChapter; nextChapter = null; $('boundary').close();
  if (destination >= book.pieces.length) {
    if (!hasNextWindow()) { persist(); returnToText(); notice('End of book.'); return; }
    if (!await moveWindow(windowInfo.start + book.pieces.length) || epoch !== playbackEpoch) return;
  } else at = destination;
  displayCurrent();
  if (enterNote(true)) return;
  if (['image', 'unsupported'].includes(kindAt(at))) { mode = 'paused'; return; }
  beginPlayback();
}
function showDetails(title, text, history = false) {
  $('details-title').textContent = title; $('details-body').textContent = text; $('clear-history').hidden = !history; $('details').showModal();
}
async function loadBook(value) {
  navigationEpoch++;
  const validated = validateBook(value);
  stop();
  imageEpoch++; for (const img of imageCache.values()) releaseImage(img); imageCache.clear();
  await writeQueue;
  const prefsForBook = saved.books[validated.book_id];
  const prepared = native ? await native.invoke('prepare_session', { book: validated, saved: prefsForBook ?? null }) : null;
  policy = prepared; windowInfo = prepared?.window ?? null;
  comfort = new Comfort(policy?.comfort); exposures = []; exposure = null; vocabulary.reset();
  notePlayback = null; book = prepared ? validateBook(prepared.book) : validated; mode = 'reading'; $('playback').hidden = true; guarded = false;
  if (prepared?.preferences) saved.books[book.book_id] = prepared.preferences;
  const prefs = preferences(), location = prepared?.resume ?? restore(book, prefs.revision ? prefs : null);
  $('speed').value = Number.isFinite(prefs.speed) ? prefs.speed : 350;
  $('font-size').value = Number.isFinite(prefs.fontSize) ? prefs.fontSize : 20;
  $('feedback-enabled').checked = prefs.feedback !== false;
  $('follow-notes').checked = prefs.followNotes === true;
  at = location.kind === 'exact' ? location.at : 0;
  paceChanges = prefs.pendingChapter === (chapterAt(book, at)?.id ?? null) && Array.isArray(prefs.pendingChanges) ? prefs.pendingChanges : [];
  guarded = !['fresh', 'exact'].includes(location.kind);
  render(); reflow(); notice(prepared?.vocabulary_error ? 'Book opened; vocabulary unavailable: ' + prepared.vocabulary_error : 'Select a passage to choose a starting point. Space plays or pauses; Escape returns.');
  if (guarded) {
    recoveryAt = location.kind === 'unresolved' ? null : location.at;
    $('recovery-text').textContent = location.kind === 'passage' ? 'One matching passage was found. Confirm before using it.' : location.kind === 'chapter' ? 'The old passage was not uniquely found. Use the identified chapter start?' : 'No unique passage or identified chapter was found. Your old saved location is preserved. Choose a new passage to replace it.';
    $('accept-recovery').hidden = recoveryAt === null; $('recovery').showModal();
  }
}
async function importBook(json) {
  stop(); mode = 'reading'; navigationEpoch++; persist(true);
  const parsed = validateBook(JSON.parse(json));
  await writeQueue;
  const value = native ? await native.invoke('import_book', { json }) : parsed;
  await loadBook(value);
}
$('open').addEventListener('change', async event => {
  const file = event.target.files[0]; if (!file) return;
  try {
    stop(); mode = 'reading'; navigationEpoch++; persist(true);
    await writeQueue;
    if (native) {
      if (file.size > 1024 * 1024 * 1024) throw new Error('Book exceeds 1 GiB');
      await native.invoke('upload_book', {data: [], start: true, finish: false});
      for (let offset = 0; offset < file.size; offset += 65536) {
        const data = Array.from(new Uint8Array(await file.slice(offset, offset + 65536).arrayBuffer()));
        await native.invoke('upload_book', {data, start: false, finish: false});
      }
      const value = await native.invoke('upload_book', {data: [], start: false, finish: true});
      await loadBook(value);
    } else {
      if (file.size > 32 * 1024 * 1024) throw new Error('Browser preview limit is 32 MiB');
      await importBook(await file.text());
    }
  } catch (error) { notice(`Could not open this book: ${error}`); }
  event.target.value = '';
});
$('book').addEventListener('mouseup', event => {
  const selection = window.getSelection(); let element;
  if (selection && !selection.isCollapsed && selection.rangeCount) {
    const range = selection.getRangeAt(0);
    const container = range.startContainer.nodeType === Node.TEXT_NODE ? range.startContainer.parentElement : range.startContainer;
    element = container.closest?.('[data-piece]');
    if (!element) element = [...$('book').querySelectorAll('[data-piece]')].find(el => range.intersectsNode(el));
  } else element = event.target.closest('[data-piece]');
  if (element) { choose(Number(element.dataset.piece)); notice('Passage selected. Space starts here.'); }
});
$('play').onclick = play; $('return').onclick = returnToText;
$('previous').onclick = () => turn(-1); $('next').onclick = () => turn(1);
$('settings-button').onclick = () => {
  const hidden = !$('settings').hidden;
  $('settings').hidden = hidden;
  $('book-controls').hidden = hidden;
  $('settings-button').setAttribute('aria-expanded', String(!hidden));
  reflow();
};
$('font-size').oninput = reflow;
$('speed').onchange = () => { $('speed').value = speed(); paceChanges.push({ date: new Date().toISOString(), wpm: speed(), sources: book.pieces[at].sources }); persist(); };
$('feedback-enabled').onchange = persist;
$('follow-notes').onchange = persist;
$('chapters').onchange = async () => {
  const destination = Number($('chapters').value);
  returnToText();
  if (windowInfo) { await moveWindow(destination); choose(at); }
  else { choose(destination); reflow(); }
};
$('pace').onchange = () => { const delta = $('pace').value === 'fast' ? -25 : 25; $('suggestion').hidden = $('pace').value === 'comfortable'; $('accept-speed').checked = false; $('suggestion-text').textContent = `Use ${Math.max(60, Math.min(1200, speed() + delta))} WPM for the next chapter`; };
$('skip-feedback').onclick = () => continueChapter(false); $('save-feedback').onclick = () => continueChapter(true);
$('boundary').addEventListener('close', () => { if (nextChapter !== null) { nextChapter = null; returnToText(); } });
$('history-button').onclick = () => showDetails('This book’s feedback', (preferences().history ?? []).map(h => `${h.date}\nChapter: ${h.chapter}\nPace: ${h.pace}; followed: ${h.understanding}\nSpeed: ${h.speedBefore} → ${h.speedAfter} WPM\nDuring chapter: ${h.changes.map(c => c.wpm).join(', ') || 'no changes'}`).join('\n\n') || 'No chapter feedback saved.', true);
$('clear-history').onclick = () => { saved.books[book.book_id] = { ...preferences(), history: [] }; persist(); $('details-body').textContent = 'History cleared.'; };
$('close-details').onclick = () => $('details').close();
$('accept-recovery').onclick = () => { if (recoveryAt !== null) { choose(recoveryAt); reflow(); } $('recovery').close(); };
$('cancel-recovery').onclick = () => { $('recovery').close(); notice('Old saved location preserved. Select a passage or turn a page to choose a new location.'); };
document.addEventListener('keydown', event => {
  if (document.querySelector('dialog[open]')) return;
  if (mode === 'reading' && /INPUT|SELECT|TEXTAREA/.test(event.target.tagName)) return;
  if (mode === 'reading' && event.target.tagName === 'BUTTON' && event.code === 'Space') return;
  if (event.code === 'KeyC') { event.preventDefault(); vocabulary.capture(); }
  else if (event.code === 'KeyV') { event.preventDefault(); vocabulary.open(); }
  else if (event.code === 'Space') { event.preventDefault(); play(); }
  else if (event.code === 'Escape') returnToText();
  else if (event.code === 'ArrowRight') { event.preventDefault(); turn(1); }
  else if (event.code === 'ArrowLeft') { event.preventDefault(); turn(-1); }
});
new ResizeObserver(() => { if (mode === 'reading') reflow(); }).observe($('viewport'));
window.addEventListener('pagehide', () => persist(true));
document.addEventListener('visibilitychange', () => { if (document.hidden && mode === 'playing') { stop(); mode = 'paused'; persist(); } });
export const reader = {
  get startup() { return startup; },
  get snapshot() { return { book, windowInfo, globalAt: (windowInfo?.start ?? 0) + at, at, page, pages, mode, guarded, notePlayback, state: saved, speed: speed(), pageForPiece }; },
  async seekGlobal(index) {
    if (!windowInfo) { choose(index); return; }
    if (index < windowInfo.start || index >= windowInfo.start + book.pieces.length) {
      if (!await moveWindow(index)) throw new Error('Could not seek to passage');
    }
    choose(index - windowInfo.start);
  },
  choose, reflow, turn, play, returnToText, importBook, loadBook, advance, vocabulary,
  flush: () => writeQueue,
};
async function boot() {
  await startTheme(native);
  let result;
  if (native) result = await native.invoke('boot');
  else {
    let stored;
    try { stored = JSON.parse(localStorage.getItem(storageKey)); } catch { notice('Browser scratch state could not be read.'); }
    result = { book: stored?.book ?? await (await fetch('demo.json')).json(), state: stored?.state };
  }
  saved = { books: Object.assign(Object.create(null), result.state?.books ?? {}) };
  await loadBook(result.book);
  startup = JSON.parse(JSON.stringify(saved));
  if (result.warning) notice(result.warning);
  if (result.self_test || new URLSearchParams(location.search).has('self-test')) {
    const { runTests } = await import(result.memory_test ? './memory-test.js' : result.theme_test ? './theme-test.js' : './self-test.js');
    const report = await runTests(reader);
    await writeQueue;
    if (native) await native.invoke('test_result', { result: report });
    else { document.body.dataset.testResult = JSON.stringify(report); notice(JSON.stringify(report)); }
  }
}
boot().catch(() => notice('The reader could not start. Check the supplied JSON and scratch directory.'));
