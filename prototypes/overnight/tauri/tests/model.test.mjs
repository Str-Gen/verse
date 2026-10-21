import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { validateBook, sourceIndex, runSegments, restore, position, utf16ToScalar, localImage } from '../web/model.js';
const book = validateBook(JSON.parse(await readFile(new URL('../web/demo.json', import.meta.url))));
test('all normalized scalar runs retain text through piece mapping', () => {
  const index = sourceIndex(book);
  for (const doc of book.documents) for (const block of doc.blocks) for (const run of block.runs) assert.equal(runSegments(run,index).map(s=>s.text).join(''),run.text);
});
test('joined inline word maps all three fragments to one piece', () => {
  const at = book.pieces.findIndex(p=>p.text==='unbelievable');
  const block=book.documents[0].blocks.find(b=>b.id===book.pieces[at].block_id);
  assert.equal(block.runs.flatMap(r=>runSegments(r,sourceIndex(book))).filter(s=>s.piece===at).map(s=>s.text).join(''),'unbelievable');
  assert.equal(utf16ToScalar('A🦉B',3),2);
});
test('revision changes demand confirmation, duplicate passage falls back to identified chapter', () => {
  const at=book.pieces.findIndex(p=>p.text==='unbelievable'); const saved=position(book,at);
  assert.deepEqual(restore(book,saved),{kind:'exact',at});
  const revised=structuredClone(book); revised.revision='changed';
  assert.equal(restore(revised,saved).kind,'passage');
  revised.pieces.push(...structuredClone(revised.pieces));
  assert.equal(restore(revised,saved).kind,'chapter');
  assert.equal(restore(revised,{...saved,chapter:'absent'}).kind,'unresolved');
});
test('bad schema and invalid source ranges fail before replacing book', () => {
  assert.throws(()=>validateBook({schema_version:9}));
  const bad=structuredClone(book); bad.pieces[0].sources[0].offset=-1;
  assert.throws(()=>validateBook(bad));
});
test('split compound pieces retain separate exact source anchors', () => {
  const split = structuredClone(book);
  const at = split.pieces.findIndex(p => p.text === 'unbelievable');
  const original = split.pieces[at];
  const first = {...original, id:'split-first', text:'un', sources:[original.sources[0]]};
  const second = {...original, id:'split-second', text:'believable', sources:original.sources.slice(1)};
  split.pieces.splice(at,1,first,second);
  validateBook(split);
  const block = split.documents[0].blocks.find(b=>b.id===original.block_id);
  const fragments = block.runs.flatMap(r=>runSegments(r,sourceIndex(split)));
  assert.equal(fragments.filter(s=>s.piece===at).map(s=>s.text).join(''),'un');
  assert.equal(fragments.filter(s=>s.piece===at+1).map(s=>s.text).join(''),'believable');
  assert.deepEqual(restore(split,position(split,at+1)),{kind:'exact',at:at+1});
});

test('large embedded images reach the renderer while external and executable URLs stay rejected', () => {
  const large = 'data:image/png;base64,' + 'A'.repeat(8_000_004);
  assert.equal(localImage(large), large);
  for (const value of ['https://example.com/image.png', 'file:///tmp/image.png', 'javascript:alert(1)', 'data:image/svg+xml;base64,AAAA']) {
    assert.equal(localImage(value), null);
  }
});
