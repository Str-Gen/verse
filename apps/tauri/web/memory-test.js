const wait = ms => new Promise(resolve => setTimeout(resolve, ms));
export async function runTests(reader) {
  const errors = [], native = window.__TAURI__.core;
  const total = reader.snapshot.windowInfo.total;
  let windows = 0, maxPieces = 0, maxNodes = 0;
  await document.fonts.ready;
  await native.invoke('test_phase', {phase: 'reading'});
  await wait(300);
  for (let i = 0; i < 40; i++) {
    const target = Math.floor((i % 2 ? 1 - i / 40 : i / 40) * (total - 1));
    await reader.seekGlobal(target); reader.reflow();
    const state = reader.snapshot;
    if (state.globalAt !== target) errors.push(`seek ${target} reached ${state.globalAt}`);
    maxPieces = Math.max(maxPieces, state.book.pieces.length);
    maxNodes = Math.max(maxNodes, document.querySelectorAll('#book *').length);
    if (state.book.pieces.length > 512) errors.push('Text window exceeded 512 pieces');
    windows++;
    await wait(50);
  }
  await reader.flush();
  await wait(300);
  return {passed: errors.length === 0, errors, total, windows, maxPieces, maxNodes};
}
