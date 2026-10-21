// Source offsets count Unicode scalars, unlike DOM Range offsets, which count UTF-16 units.
export const scalars = text => Array.from(text);
export const utf16ToScalar = (text, offset) => scalars(text.slice(0, offset)).length;
export const scalarToUtf16 = (text, offset) => scalars(text).slice(0, offset).join('').length;
const record = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const string = value => typeof value === 'string';
const integer = value => Number.isSafeInteger(value) && value >= 0;
const span = value => record(value) && string(value.document) && string(value.node) && integer(value.offset) && integer(value.length);
export function validateBook(book) {
  const fail = () => { throw new Error('Invalid version-1 book JSON. The current book has been kept.'); };
  if (!record(book) || book.schema_version !== 1 || !string(book.book_id) || !book.book_id || !string(book.revision) || !string(book.title) || !Array.isArray(book.documents) || !Array.isArray(book.pieces) || !book.pieces.length || !Array.isArray(book.chapters) || !Array.isArray(book.notes)) fail();
  const blocks = new Set(), documents = new Set(), pieces = new Set();
  for (const doc of book.documents) {
    if (!record(doc) || !string(doc.id) || documents.has(doc.id) || !Array.isArray(doc.blocks)) fail();
    documents.add(doc.id);
    for (const block of doc.blocks) {
      if (!record(block) || !string(block.id) || blocks.has(block.id) || !string(block.kind) || !Array.isArray(block.runs)) fail();
      blocks.add(block.id);
      for (const run of block.runs) {
        if (!record(run) || !string(run.text) || !Array.isArray(run.sources) || !run.sources.every(span)) fail();
        if (run.sources.reduce((n, s) => n + s.length, 0) !== scalars(run.text).length) fail();
      }
    }
  }
  for (const piece of book.pieces) {
    if (!record(piece) || !string(piece.id) || pieces.has(piece.id) || !string(piece.text) || !blocks.has(piece.block_id) || !Array.isArray(piece.sources) || !piece.sources.every(span) || piece.sources.some(s => !documents.has(s.document))) fail();
    pieces.add(piece.id);
  }
  for (const chapter of book.chapters) {
    if (!record(chapter) || !string(chapter.id) || !string(chapter.title) || !pieces.has(chapter.start_piece) || (chapter.end_piece_exclusive !== null && !pieces.has(chapter.end_piece_exclusive))) fail();
  }
  for (const note of book.notes) {
    if (!record(note) || !string(note.id) || !Array.isArray(note.blocks) || !Array.isArray(note.pieces)) fail();
    for (const block of note.blocks) {
      if (!record(block) || !Array.isArray(block.runs) || !block.runs.every(r => record(r) && string(r.text))) fail();
    }
    for (const piece of note.pieces) {
      if (!record(piece) || !string(piece.text) || !Array.isArray(piece.sources) || !piece.sources.every(span)) fail();
    }
  }
  if (book.resources !== undefined && (!Array.isArray(book.resources) || !book.resources.every(r => record(r) && string(r.id) && string(r.data_url)))) fail();
  return book;
}
export const sourceKey = source => JSON.stringify([source.document, source.node]);
export function sourceIndex(book) {
  const index = new Map();
  book.pieces.forEach((piece, pieceIndex) => {
    for (const source of piece.sources) {
      const key = sourceKey(source);
      if (!index.has(key)) index.set(key, []);
      index.get(key).push({ ...source, pieceIndex });
    }
  });
  return index;
}
export function runSegments(run, index) {
  const chars = scalars(run.text), owners = Array(chars.length).fill(null);
  let base = 0;
  for (const source of run.sources) {
    for (const candidate of index.get(sourceKey(source)) ?? []) {
      const start = Math.max(source.offset, candidate.offset);
      const end = Math.min(source.offset + source.length, candidate.offset + candidate.length);
      for (let at = start; at < end; at++) owners[base + at - source.offset] = candidate.pieceIndex;
    }
    base += source.length;
  }
  const result = [];
  chars.forEach((char, i) => {
    const previous = result.at(-1);
    if (previous && previous.piece === owners[i]) previous.text += char;
    else result.push({ text: char, piece: owners[i] });
  });
  return result;
}
export const sameSources = (a, b) => a.length === b.length && a.every((s, i) => s.document === b[i].document && s.node === b[i].node && s.offset === b[i].offset && s.length === b[i].length);
export function chapterAt(book, at) {
  const ids = new Map(book.pieces.map((p, i) => [p.id, i]));
  return book.chapters.find(c => at >= ids.get(c.start_piece) && at < (c.end_piece_exclusive === null ? book.pieces.length : ids.get(c.end_piece_exclusive)));
}
export function contextAt(book, at) { return book.pieces.slice(Math.max(0, at - 2), at + 3).map(p => p.text).join('\u001f'); }
export function restore(book, saved) {
  if (!saved) return { kind: 'fresh', at: 0 };
  if (saved.revision === book.revision && Array.isArray(saved.sources) && saved.sources.length) {
    const at = book.pieces.findIndex(p => sameSources(p.sources, saved.sources));
    if (at >= 0) return { kind: 'exact', at };
  }
  const matches = book.pieces.map((_, i) => i).filter(i => contextAt(book, i) === saved.context);
  if (matches.length === 1 && saved.context?.length >= 20) return { kind: 'passage', at: matches[0] };
  const chapters = book.chapters.filter(c => c.id === saved.chapter);
  if (chapters.length === 1) return { kind: 'chapter', at: book.pieces.findIndex(p => p.id === chapters[0].start_piece) };
  return { kind: 'unresolved', at: 0 };
}
export function position(book, at) {
  return { revision: book.revision, sources: book.pieces[at].sources, context: contextAt(book, at), chapter: chapterAt(book, at)?.id ?? null };
}
