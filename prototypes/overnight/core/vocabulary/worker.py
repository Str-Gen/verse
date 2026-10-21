"""Private JSON-lines adapter for the accepted vocabulary module. No HTTP server."""
import json
import os
from pathlib import Path
import re
import sys
import uuid

from vocabulary import Jev, VocabularyCapture
from dictionary import lookup


def document(book):
    paragraphs, pieces, indices = [], [], {}
    sentence = 0
    by_block = {}
    for index, piece in enumerate(book['pieces']):
        if piece['kind'] == 'word':
            by_block.setdefault(piece['block_id'], []).append((index, piece))
    for doc in book['documents']:
        for block in doc['blocks']:
            text, spans = '', []
            for run in block['runs']:
                offset = len(text)
                text += run['text']
                for source in run['sources']:
                    spans.append((source, offset))
                    offset += source['length']
            paragraph = len(paragraphs)
            paragraphs.append(text)
            for index, piece in by_block.get(block['id'], []):
                offsets = []
                for source in piece['sources']:
                    for span, base in spans:
                        if source['document'] == span['document'] and source['node'] == span['node']:
                            start = max(source['offset'], span['offset'])
                            end = min(source['offset'] + source['length'], span['offset'] + span['length'])
                            if start < end:
                                offsets.append((base + start - span['offset'], base + end - span['offset']))
                if not offsets:
                    raise ValueError('Vocabulary source position is missing.')
                indices[index] = len(pieces)
                pieces.append(dict(paragraph=paragraph, start=min(s for s, _ in offsets), end=max(e for _, e in offsets), sentence=sentence))
                if re.search(r'[.!?][\"\u201d\u2019\')]*$', piece['text']):
                    sentence += 1
            sentence += 1
    return paragraphs, pieces, indices


def main():
    os.umask(0o077)
    root = Path(sys.argv[1])
    root.mkdir(parents=True, exist_ok=True)
    legacy = Path.home() / 'speedreading/omawrite/test-data/local'
    dictionary = Path(os.environ.get('VERSE_DICTIONARY', legacy / 'dictionary.sqlite')).resolve()
    key = os.environ.get('TYPESAFE_API_KEY', '')
    key_file = Path(os.environ.get('VERSE_KEY_FILE', Path.home() / 'speedreading/jev-speedreading.key'))
    if not key and key_file.is_file():
        key = key_file.read_text().strip()
    notebook = VocabularyCapture(root / 'vocabulary.sqlite', Jev(key) if key else None)
    current, indices = None, {}
    try:
        for line in sys.stdin:
            try:
                request = json.loads(line)
                action = request['action']
                if action == 'open':
                    paragraphs, pieces, indices = document(request['book'])
                    current = notebook.open_document(paragraphs, pieces, request['book']['title'], request['book'].get('book_id'))
                    result = {'document': current}
                elif action == 'capture':
                    exposures = [{**e, 'piece': indices[e['piece']]} for e in request['exposures'] if e['piece'] in indices]
                    result = notebook.capture(current, exposures, request['now'], request['scope'], request.get('tap_id') or uuid.uuid4().hex)
                elif action == 'review':
                    result = notebook.review(current)
                elif action == 'feedback':
                    notebook.feedback(request['capture'], request['candidate'], request['choice'])
                    result = notebook.review(current)
                elif action == 'retry':
                    notebook.retry(request['capture'])
                    result = notebook.review(current)
                elif action == 'definition':
                    result = lookup(dictionary, request['word'])
                else:
                    raise ValueError('Unknown vocabulary action.')
                print(json.dumps({'result': result}), flush=True)
            except Exception as error:
                print(json.dumps({'error': str(error)}), flush=True)
    finally:
        notebook.close()


if __name__ == '__main__':
    main()
