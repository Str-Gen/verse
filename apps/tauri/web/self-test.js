import { utf16ToScalar, scalarToUtf16, runSegments, sourceIndex, restore, position, validateBook } from './model.js';
const wait = ms => new Promise(resolve => setTimeout(resolve, ms));
const $ = id => document.getElementById(id);
export async function runTests(actualReader) {
  const native = Boolean(window.__TAURI__?.core);
  const reader = native ? {
    ...actualReader,
    get snapshot() {
      const state = actualReader.snapshot, offset = state.windowInfo?.start ?? 0;
      return {...state, at: state.globalAt, pageForPiece: new Map([...state.pageForPiece].map(([i,p]) => [i + offset,p]))};
    },
    choose: index => actualReader.seekGlobal(index),
    loadBook: value => actualReader.importBook(JSON.stringify(value)),
  } : actualReader;
  const checks = [], errors = [];
  function check(name, condition) { checks.push({ name, passed: Boolean(condition) }); if (!condition) errors.push(name + (name.includes('image') || name.includes('raster') ? ': ' + JSON.stringify({notice: $('notice')?.textContent, images: [...$('book').querySelectorAll('figure')].map(el => el.outerHTML)}) : '')); }
  try {
    await document.fonts.ready; await wait(100);
    check('quiet controls start collapsed', $('settings').hidden && $('book-controls').hidden && $('settings-button').getAttribute('aria-expanded') === 'false');
    check('reading uses Omawrite font family', getComputedStyle($('book')).fontFamily.includes('IBM Plex Mono'));
    $('settings-button').click();
    check('footer menu exposes file and reading controls', !$('settings').hidden && !$('book-controls').hidden);
    const sidebar = $('settings').getBoundingClientRect(), readingStage = $('reading-stage').getBoundingClientRect();
    check('contents occupy a left sidebar beside the reading stage', sidebar.right <= readingStage.left && sidebar.height > sidebar.width);
    const progressBox = $('book-progress').getBoundingClientRect();
    check('book progress sits at bottom center', Math.abs((progressBox.left + progressBox.right) / 2 - innerWidth / 2) < 1 && progressBox.bottom === innerHeight);

    $('speed').dispatchEvent(new KeyboardEvent('keydown', {code: 'Space', bubbles: true}));
    check('Space dismisses controls without starting playback', reader.snapshot.mode === 'reading');
    check('footer menu collapses auxiliary controls', $('settings').hidden && $('book-controls').hidden);
    const original = native ? await (await fetch('demo.json')).json() : JSON.parse(JSON.stringify(reader.snapshot.book));
    check('startup is stopped at a source-matching saved anchor', reader.snapshot.mode === 'reading' && restore(original, reader.startup.books[original.book_id]).at === reader.snapshot.at);
    check('native or browser DOM renders normalized book', $('book').textContent.includes('observatory'));
    check('long synthetic book has multiple conventional pages', reader.snapshot.pages > 1);
    check('UTF-16 offsets convert to Unicode scalar source offsets', utf16ToScalar('A🦉B', 3) === 2 && scalarToUtf16('A🦉B', 2) === 3);
    const joined = original.pieces.findIndex(p => p.text === 'unbelievable');
    check('inline formatting retains one lexical piece with multiple source spans', joined >= 0 && original.pieces[joined].sources.length === 3 && document.querySelectorAll(`[data-piece="${joined}"]`).length === 3);
    const fragment = document.querySelector(`[data-piece="${joined}"] em`) ?? [...document.querySelectorAll(`[data-piece="${joined}"]`)].find(el => el.closest('em'));
    check('normalized emphasis rendered', Boolean(fragment));
    const target = document.querySelector(`[data-piece="${joined}"]`);
    const range = document.createRange(); range.setStart(target.firstChild, 1); range.setEnd(target.firstChild, 2);
    window.getSelection().removeAllRanges(); window.getSelection().addRange(range);
    target.dispatchEvent(new MouseEvent('mouseup', { bubbles: true }));
    check('DOM passage selection maps to source piece', reader.snapshot.at === joined);
    window.getSelection().removeAllRanges();
    reader.play();
    check('preparation starts with a pulse', $('playback').classList.contains('preparing'));
    check('playback starts on selected passage', $('word').textContent === 'unbelievable' && reader.snapshot.mode === 'playing');
    await wait(0);
    check('playback keeps footer controls and progress visible', !$('return').hidden && $('play').textContent === 'Pause' && $('book-progress').getBoundingClientRect().height > 0);

    reader.play(); const paused = reader.snapshot.at; await wait(300);
    check('pause stops advancement', reader.snapshot.mode === 'paused' && reader.snapshot.at === paused);
    reader.returnToText();
    check('return selects last displayed passage', reader.snapshot.at === joined && reader.snapshot.mode === 'reading' && $('playback').hidden);
    $('play').focus(); $('play').click();
    document.activeElement.dispatchEvent(new KeyboardEvent('keydown', { code: 'Escape', bubbles: true }));
    check('Escape returns after playback starts with a focused button', reader.snapshot.mode === 'reading');
    $('play').blur();
    $('speed').value = '60';
    await reader.choose(joined); reader.play();
    await wait(500);
    check('preparation holds its initial source for the pulse', reader.snapshot.at === joined && $('playback').classList.contains('preparing'));
    await reader.vocabulary.capture();
    check('capture before playback exposure fails visibly', $('notice').textContent.includes('Read a few words'));
    await wait(1520);
    check('preparation ends before advancing the first word', reader.snapshot.at === joined && !$('playback').classList.contains('preparing'));
    reader.play(); reader.play();
    check('brief resume skips preparation', !$('playback').classList.contains('preparing'));
    await wait(30); reader.play();
    await reader.vocabulary.capture();
    reader.vocabulary.open(); await wait(150);
    check('native offline capture opens book review', $('vocabulary').open && $('vocabulary-status').textContent.includes('captures'));
    const actualRequest = reader.vocabulary.request;
    const review = await actualRequest({action:'review'}), capture = review.captures.at(-1);
    check('preparation excluded from captured text', capture.context === 'unbelievable');
    await actualRequest({action:'feedback',capture:capture.id,candidate:0,choice:'recognition'});
    await reader.vocabulary.refresh();
    $('practice-start').click();
    check('practice displays a saved word', $('practice').open && $('practice-word').textContent === 'unbelievable');
    await wait(1050);
    check('practice hides word after selected WPM interval', $('practice-word').textContent === '•••');
    await reader.vocabulary.reveal();
    check('practice reveals local captured context', $('practice-context').textContent.includes('unbelievable'));
    $('practice-difficult').click();
    check('practice feedback counts are temporary session feedback', $('practice-counts').textContent.includes('Difficult 1'));
    $('practice-close').click(); $('vocabulary-close').click(); reader.returnToText();
    await reader.choose(400); reader.reflow(); const sourceBefore = JSON.stringify(position(original, reader.snapshot.at));
    $('font-size').value = '29'; $('font-size').dispatchEvent(new Event('input'));
    check('font reflow preserves source position and its page', JSON.stringify(position(original, reader.snapshot.at)) === sourceBefore && reader.snapshot.page === reader.snapshot.pageForPiece.get(reader.snapshot.at));
    $('viewport').style.maxWidth = '390px'; reader.reflow(); await wait(30);
    check('narrow reflow preserves selected source', JSON.stringify(position(original, reader.snapshot.at)) === sourceBefore && reader.snapshot.page === reader.snapshot.pageForPiece.get(reader.snapshot.at));
    $('viewport').style.maxWidth = ''; $('font-size').value = '22'; reader.reflow();
    const oldPage = reader.snapshot.page, oldAt = reader.snapshot.at; await reader.turn(1);
    check('next page saves its first source piece', reader.snapshot.at > oldAt && reader.snapshot.pageForPiece.get(reader.snapshot.at) === reader.snapshot.page);
    const nextAt = reader.snapshot.at; await reader.turn(-1); check('previous page returns', reader.snapshot.at < nextAt);
    $('speed').value = '425'; $('speed').dispatchEvent(new Event('change')); await reader.choose(joined); await reader.flush();
    await reader.loadBook(original);
    check('same book restores exact source and saved speed stopped', reader.snapshot.at === joined && reader.snapshot.speed === 425 && reader.snapshot.mode === 'reading');
    const other = structuredClone(original); other.book_id = 'another-synthetic-book';
    await reader.loadBook(other); check('another book starts at default speed', reader.snapshot.speed === 350 && reader.snapshot.at === 0);
    await reader.loadBook(original); check('switching books restores independent position and speed', reader.snapshot.at === joined && reader.snapshot.speed === 425);
    try { await reader.importBook('{"schema_version":99}'); } catch { /* Expected invalid input. */ }
    check('invalid import retains current book', reader.snapshot.book.book_id === original.book_id && reader.snapshot.at === joined);
    const image = original.pieces.findIndex(p => p.kind === 'image');
    await reader.choose(image - 1); reader.play(); await reader.advance();
    check('real image pauses playback at its source item', reader.snapshot.at === image && reader.snapshot.mode === 'paused');
    reader.play(); check('image continuation reaches following text', reader.snapshot.at === image + 1); reader.returnToText();
    if (native) {
      await reader.choose(image); reader.returnToText(); await reader.flush();
      await reader.loadBook(original);
      check('source-less image position survives native save and reopen stopped', reader.snapshot.at === image && reader.snapshot.mode === 'reading');
    }
    const unsupported = original.pieces.findIndex(p => p.kind === 'unsupported');
    await reader.choose(unsupported); reader.play(); check('unsupported content pauses visibly', reader.snapshot.mode === 'paused' && $('word').textContent.includes('Synthetic table')); reader.returnToText();
    const second = original.pieces.findIndex(p => p.id === original.chapters[1].start_piece);
    await reader.choose(second - 1); reader.play(); await reader.advance();
    check('logical chapter boundary pauses', $('boundary').open && reader.snapshot.at === second - 1);
    const count = reader.snapshot.state.books[original.book_id].history.length;
    $('skip-feedback').click(); await wait(60); reader.play();
    check('skip starts next chapter without saving feedback', reader.snapshot.at === second && reader.snapshot.state.books[original.book_id].history.length === count); reader.returnToText();
    await reader.choose(second - 1); reader.play(); await reader.advance(); $('pace').value = 'fast'; $('pace').dispatchEvent(new Event('change')); $('accept-speed').checked = true; $('save-feedback').click(); await wait(60); reader.play();
    check('explicit feedback acceptance reduces speed by 25', reader.snapshot.speed === 400 && reader.snapshot.state.books[original.book_id].history.length === count + 1); reader.returnToText();
    const decoration = original.pieces.findIndex(p => p.kind === 'ornament'); await reader.choose(decoration - 1); reader.play(); await reader.advance();
    check('decorations are skipped', reader.snapshot.at === decoration + 1); reader.returnToText();
    const noteStart = original.pieces.findIndex(p => original.documents.flatMap(d => d.blocks).find(b => b.id === p.block_id)?.kind === 'note');
    await reader.choose(noteStart - 1); reader.play(); await reader.advance();
    check('notes skipped by default and next unsupported item still pauses', reader.snapshot.at === unsupported && reader.snapshot.mode === 'paused'); reader.returnToText();
    await reader.choose(joined); const location = position(original, joined);
    const edited = structuredClone(original); edited.revision = 'synthetic-v2';
    check('changed revision offers chapter start confirmation', restore(edited, location).kind === 'chapter');
    await reader.loadBook(edited); check('recovery guards saved state until confirmation', $('recovery').open && reader.snapshot.guarded && reader.snapshot.state.books[original.book_id].revision === original.revision);
    $('cancel-recovery').click(); await reader.flush();
    check('declining recovery preserves old revision', reader.snapshot.state.books[original.book_id].revision === original.revision);
    await reader.loadBook(edited); $('accept-recovery').click(); check('confirmed recovery uses chapter start', reader.snapshot.at === restore(edited, location).at && !reader.snapshot.guarded);
    check('unresolved recovery never guesses an index', restore(edited, { ...location, context: 'missing context', chapter: 'missing-chapter' }).kind === 'unresolved');
    const malicious = structuredClone(original); malicious.title = '<script>window.publisherRan=true</script>'; malicious.documents[0].blocks[0].runs[0].text = '<script>bad()</script>'; malicious.documents[0].blocks[0].runs[0].sources[0].length = Array.from(malicious.documents[0].blocks[0].runs[0].text).length;
    malicious.documents[0].blocks.find(b => b.kind === 'image').resource = 'https://example.invalid/tracker.png'; malicious.book_id = 'security-synthetic';
    await reader.loadBook(malicious);
    await reader.choose(malicious.pieces.findIndex(p => p.kind === 'image'));
    check('publisher markup remains text and external image URL is not rendered', !window.publisherRan && !$('book').querySelector('script') && !$('book').querySelector('img[src^="https:"]'));
    await wait(300);
    check('missing native images retain an explicit placeholder', [...$('book').querySelectorAll('figcaption')].some(el => el.textContent.includes('Image unavailable')));
    const core = validateBook(await (await fetch('core-demo.json')).json());
    await reader.importBook(JSON.stringify(core));
    check('Rust-core synthetic export opens through the JSON adapter', reader.snapshot.book.pieces.length === core.pieces.length && reader.snapshot.book.chapters.length === 3 && reader.snapshot.book.pieces.some(p => p.text === 'state-'));
    await wait(500);
    check('core embedded raster resources render locally', [...$('book').querySelectorAll('img')].length === 5 && [...$('book').querySelectorAll('img[src]')].length > 0 && [...$('book').querySelectorAll('img[src]')].every(img => img.complete && img.naturalWidth > 0 && (native ? img.src.startsWith('blob:') : img.src.startsWith('data:image/'))));
    check('loaded images do not duplicate alt text as captions', !$('book').querySelector('figcaption'));
    const imageSource = JSON.stringify(position(core, reader.snapshot.at));
    for (const limit of ['', '30px']) {
      $('viewport').style.maxWidth = limit; reader.reflow(); await wait(30);
      check(`images preserve proportions without upscaling at width ${limit || 'normal'}`, [...$('book').querySelectorAll('img[src]')].every(img => {
        const rect = img.getBoundingClientRect();
        return rect.width > 0 && rect.height > 0 && rect.width <= img.naturalWidth + 0.1 && rect.height <= img.naturalHeight + 0.1 && Math.abs(rect.width / rect.height - img.naturalWidth / img.naturalHeight) < 0.02 && rect.width <= $('viewport').clientWidth + 0.1;
      }));
    }
    $('viewport').style.maxWidth = ''; reader.reflow();
    check('image reflow preserves source anchor', JSON.stringify(position(core, reader.snapshot.at)) === imageSource);

    const coreJoined = core.pieces.findIndex(p => p.text === 'naïveté');
    check('core multi-run source piece maps to three conventional fragments', document.querySelectorAll(`[data-piece="${coreJoined}"]`).length === 3);
    const marker = core.pieces.findIndex(p => p.kind === 'note');
    const link = $('book').querySelector('.note-link'); link.click();
    check('core note reference opens manual note text', $('details').open && $('details-body').textContent.includes('manual note'));
    $('close-details').click();
    $('follow-notes').checked = false; $('follow-notes').dispatchEvent(new Event('change'));
    await reader.choose(marker - 1); reader.play(); await reader.advance();
    check('core note marker skips with note following disabled', reader.snapshot.at === marker + 1); reader.returnToText();
    $('follow-notes').checked = true; $('follow-notes').dispatchEvent(new Event('change'));
    await reader.choose(marker - 1); reader.play(); await reader.advance();
    check('optional core note playback enters note content', Boolean(reader.snapshot.notePlayback) && $('word').textContent === core.notes[0].pieces[0].text);
    for (let i = 0; i < core.notes[0].pieces.length; i++) await reader.advance();
    check('note completion returns immediately after its main-text marker', reader.snapshot.at === marker + 1 && reader.snapshot.notePlayback === null);
    reader.returnToText();
    const brokenNote = core.pieces.findIndex(p => p.kind === 'unsupported' && p.note_id);
    await reader.choose(brokenNote); reader.play();
    check('unresolved core note pauses instead of disappearing', reader.snapshot.mode === 'paused'); reader.returnToText();
    const unicode = validateBook(await (await fetch('unicode.json')).json());
    await reader.importBook(JSON.stringify(unicode));
    const astral = unicode.pieces.findIndex(p => p.text === 'A😀B');
    const astralElement = document.querySelector(`[data-piece="${astral}"]`);
    const astralRange = document.createRange(); astralRange.setStart(astralElement.firstChild, 1); astralRange.setEnd(astralElement.firstChild, 3);
    window.getSelection().removeAllRanges(); window.getSelection().addRange(astralRange);
    astralElement.dispatchEvent(new MouseEvent('mouseup', { bubbles: true }));
    window.getSelection().removeAllRanges(); reader.play();
    check('Unicode fixture UTF-16 selection begins whole scalar source word', reader.snapshot.at === astral && $('word').textContent === 'A😀B');
    await reader.advance();
    check('Unicode fixture next astral word stays whole', $('word').textContent === '𝄞tone');
    reader.returnToText();
    check('Unicode fixture source return remains at last displayed word', reader.snapshot.at === astral + 1 && reader.snapshot.page === reader.snapshot.pageForPiece.get(astral + 1));
    if (native) {
      await reader.loadBook(original);
      for (const destination of [0, 700, 1100, 0, 1000, 100]) {
        await reader.choose(destination); reader.reflow();
        check(`disk window seeks exactly to ${destination}`, reader.snapshot.at === destination && reader.snapshot.book.pieces.length <= 512 && document.querySelectorAll('#book [data-piece]').length < 1600);
      }
      await reader.choose(0); await reader.loadBook(original);
      await reader.choose(511); reader.play();
      const advancing = reader.advance(); reader.returnToText(); await advancing;
      check('Escape during a disk-window transition leaves playback stopped', reader.snapshot.mode === 'reading' && $('playback').hidden);
    }
    $('follow-notes').checked = false;
    await reader.loadBook(original); if ($('recovery').open) $('accept-recovery').click(); await reader.choose(joined); $('speed').value = '425'; $('speed').dispatchEvent(new Event('change')); reader.returnToText(); await reader.flush();
  } catch (error) { errors.push(`Exception: ${error.name}: ${error.message}`); }
  return { passed: errors.length === 0, engine: navigator.userAgent, checks, errors };
}
