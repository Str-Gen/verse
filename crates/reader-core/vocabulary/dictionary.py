"""Local English definitions, imported from Kaikki Wiktextract JSONL."""
import argparse
import gzip
import hashlib
import os
import sys
import tempfile
import json
import sqlite3
import unicodedata
from datetime import datetime, timezone
from pathlib import Path

SOURCE = 'https://kaikki.org/dictionary/raw-wiktextract-data.jsonl.gz'
LICENSE = 'https://creativecommons.org/licenses/by-sa/4.0/'


def bundled_dictionary(cache=None, resources=None):
    """Expand the shipped database once; publish only a complete, verified copy."""
    if resources is None:
        resources = Path(os.environ.get('VERSE_RESOURCE_DIR', Path(__file__).resolve().parents[3] / 'assets')) / 'dictionary'
    manifest = json.loads((resources / 'manifest.json').read_text())
    digest = manifest['sha256']
    if len(digest) != 64 or any(c not in '0123456789abcdef' for c in digest):
        raise ValueError('Invalid bundled dictionary checksum')
    if cache is None:
        if sys.platform == 'win32':
            base = Path(os.environ.get('LOCALAPPDATA', Path.home() / 'AppData/Local'))
        elif sys.platform == 'darwin':
            base = Path.home() / 'Library/Caches'
        else:
            base = Path(os.environ.get('XDG_CACHE_HOME', Path.home() / '.cache'))
        cache = base / 'verse/dictionary'
    cache.mkdir(parents=True, exist_ok=True)
    destination = cache / (digest + '.sqlite')
    if destination.is_file():
        return destination
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=cache, suffix='.sqlite', delete=False) as output:
            temporary = Path(output.name)
            checksum, size = hashlib.sha256(), 0
            with gzip.open(resources / 'english.sqlite.gz', 'rb') as source:
                while chunk := source.read(1024 * 1024):
                    size += len(chunk)
                    if size > manifest['bytes']:
                        raise ValueError('Bundled dictionary exceeds its declared size')
                    checksum.update(chunk)
                    output.write(chunk)
            if size != manifest['bytes'] or checksum.hexdigest() != digest:
                raise ValueError('Bundled dictionary checksum mismatch')
            output.flush()
            os.fsync(output.fileno())
        temporary.replace(destination)
        return destination
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def normalize(word):
    return unicodedata.normalize('NFC', word).casefold().strip()


def lookup(path, word):
    if not path.is_file():
        return {'available': False, 'entries': []}
    entries, visited, pending = [], set(), [(word, True)]
    with sqlite3.connect(path.as_uri() + '?mode=ro', uri=True) as db:
        metadata = dict(db.execute('SELECT key,value FROM metadata'))
        while pending:
            spelling, follow_references = pending.pop(0)
            key = normalize(spelling)
            if key in visited:
                continue
            visited.add(key)
            matches = [json.loads(row[0]) for row in db.execute('SELECT payload FROM entries WHERE word=?', (key,))]
            exact = [entry for entry in matches if entry['word'] == spelling]
            for entry in exact or matches:
                entries.append(entry)
                if not follow_references:
                    continue
                for sense in entry['senses']:
                    for reference in sense.get('form_of', []) + sense.get('alt_of', []):
                        pending.append((reference['word'], False))
    return {'available': True, 'entries': entries, 'metadata': metadata}


def build(source, destination, source_date="unspecified"):
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = destination.with_suffix('.building.sqlite')
    temporary.unlink(missing_ok=True)
    count = 0
    with sqlite3.connect(temporary) as db:
        db.execute('CREATE TABLE entries(word TEXT NOT NULL,payload TEXT NOT NULL)')
        db.execute('CREATE TABLE metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL)')
        opener = gzip.open if source.suffix == '.gz' else open
        with opener(source, 'rt', encoding='utf-8') as stream:
            for line in stream:
                entry = json.loads(line)
                if entry.get('lang_code') != 'en':
                    continue
                compact = {k: entry[k] for k in ('word', 'pos', 'etymology_text', 'etymology_number') if k in entry}
                compact['senses'] = [{k: sense[k] for k in ('glosses', 'tags', 'raw_tags', 'form_of', 'alt_of') if k in sense} for sense in entry.get('senses', [])]
                db.execute('INSERT INTO entries VALUES (?,?)', (normalize(entry['word']), json.dumps(compact, ensure_ascii=False)))
                count += 1
                if count % 100000 == 0:
                    db.commit()
                    print(f'{count:,} English entries imported', flush=True)
        db.execute('CREATE INDEX entry_word ON entries(word)')
        db.executemany('INSERT INTO metadata VALUES (?,?)', {
            'source': SOURCE, 'source_dump_date': source_date, 'license': LICENSE, 'attribution': 'Wiktionary contributors, extracted by Kaikki / Wiktextract',
            'imported': datetime.now(timezone.utc).isoformat(), 'entries': str(count),
            'modifications': 'English entries only; definition glosses, labels, etymologies and lemma references retained; reformatted as SQLite.'
        }.items())
    temporary.replace(destination)
    print(f'Ready: {count:,} entries; {destination.stat().st_size:,} bytes', flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    parser.add_argument('destination', type=Path)
    parser.add_argument('--source-date', default='unspecified', help='Dump date reported by Kaikki for this archive')
    args = parser.parse_args()
    build(args.source, args.destination, args.source_date)
