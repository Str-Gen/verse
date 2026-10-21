#!/usr/bin/env python3
"""Translate XML node spelling and inline notes for the independent oracle.

No expected text is read. Every original text span is checked against XML before
presentation translation. ElementTree paths count elements; roxmltree paths count
all siblings, including text. Offsets remain Unicode scalars.
"""
import json
from pathlib import Path
import subprocess
import sys
from xml.dom import minidom, Node
from zipfile import ZipFile

HERE = Path(__file__).resolve().parent

def adapt(book, epub):
    mapping, texts, parents = {}, {}, {}
    with ZipFile(epub) as archive:
        for name in archive.namelist():
            if not name.endswith(('.xhtml', '.html')):
                continue
            dom = minidom.parseString(archive.read(name))
            def visit(element, raw, canonical):
                mapping[name, raw] = canonical
                texts[name, raw] = ''
                element_index = 0
                previous = None
                for index, child in enumerate(element.childNodes):
                    address = f'{raw}/{index}'
                    if child.nodeType == Node.ELEMENT_NODE:
                        destination = f'{canonical}/{element_index}'
                        visit(child, address, destination)
                        previous = destination
                        element_index += 1
                    elif child.nodeType == Node.TEXT_NODE:
                        mapping[name, address] = (previous + '/tail') if previous else (canonical + '/text')
                        texts[name, address] = child.data
                        parents[name, address] = canonical
            visit(dom.documentElement, '0', '0')
    def spans(obj, expected=None):
        actual = ''
        for source in obj.get('sources', []):
            key = source['document'], source['node']
            text = texts[key]
            a, n = source['offset'], source['length']
            assert 0 <= a <= a+n <= len(text), ('invalid range', source)
            actual += text[a:a+n]
        if expected is not None:
            assert actual == expected, ('source text mismatch', expected, actual)
    blocks = [b for d in book['documents'] for b in d['blocks']]
    for block in blocks + [b for n in book['notes'] for b in n['blocks']]:
        for run in block['runs']:
            spans(run, run['text'])
    for piece in book['pieces'] + [p for n in book['notes'] for p in n['pieces']]:
        spans(piece, piece['text'] if piece['kind'] in ('word','note') else None)
    # The oracle represents inline note references as their own conventional block.
    # Preserve the marker runs and validate their actual text spans above.
    for piece in book['pieces']:
        if piece['kind'] != 'note':
            continue
        original = next(b for b in blocks if b['id'] == piece['block_id'])
        runs = [r for r in original['runs'] if r.get('note_id') == piece['note_id']]
        original['runs'] = [r for r in original['runs'] if r not in runs]
        marker = dict(original, id=piece['id'] + ':marker', kind='note', runs=runs)
        next(d for d in book['documents'] if original in d['blocks'])['blocks'].append(marker)
        piece['block_id'] = marker['id']
        source = piece['sources'][0]
        piece['sources'] = [dict(source, node=parents[source['document'], source['node']], offset=0, length=0)]
        # This event is already in canonical element spelling.
        piece['_canonical_event'] = True
    def translate(value):
        if isinstance(value, dict):
            if 'document' in value and 'node' in value:
                value['node'] = mapping[value['document'], value['node']]
            elif value.pop('_canonical_event', False):
                for key, child in value.items():
                    if key != 'sources':
                        translate(child)
            else:
                for child in value.values():
                    translate(child)
        elif isinstance(value, list):
            for child in value:
                translate(child)
    translate(book)
    # The oracle omits decoration events; the core marks them for frontend skipping.
    book["pieces"] = [p for p in book["pieces"] if p["kind"] != "ornament"]
    return book

def main():
    args = sys.argv[1:]
    result = subprocess.run([str(HERE/'../core/target/debug/reader-core'), *args])
    if result.returncode:
        return result.returncode
    output = Path(args[args.index('--output')+1])
    epub = Path(args[args.index('--epub')+1])
    output.write_text(json.dumps(adapt(json.loads(output.read_text()), epub)))
    return 0

if __name__ == '__main__':
    sys.exit(main())
