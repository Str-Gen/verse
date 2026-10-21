import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest
import zipfile

sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from fixtures import CASES, RESTORE, generate
from validate import Invalid, source_nodes, validate_export, validate_restore


def oracle_book(name):
    """Checker unit-test input from the oracle. Never label this core evidence."""
    expectation = CASES[name]['expected']
    blocks, pieces = [], []
    words = expectation['words']
    for i,w in enumerate(words):
        sources = copy.deepcopy(w['sources'])
        nodes = source_nodes(CASES[name])
        runs = [{'text':nodes[(s['document'],s['node'])][s['offset']:s['offset']+s['length']],
                 'emphasis':s['node']=='0/1/1/0/text' and name=='inline_unicode_hidden','sources':[s]} for s in sources]
        blocks.append({'id':f'b{i}','kind':'paragraph','runs':runs})
        pieces.append({'id':f'p{i}','word_id':f'w{i}','text':w['text'],'sources':sources,'block_id':f'b{i}'})
    for i,event in reversed(list(enumerate(expectation.get('events',[])))):
        kind = event['kind']
        ident = f'{kind}{i}'
        blocks.append({'id':ident,'kind':kind,'runs':[],'resource':'landscape.svg','alt':'A hill with a trail'})
        pieces.insert(event['after_word'],{'id':ident,'kind':kind,'text':kind,'block_id':ident,'note_id':'n0',
                      'sources':[{'document':event['document'],'node':event['node'],'offset':0,'length':0}]})
    for i in range(expectation.get('ornament_blocks',0)):
        blocks.append({'id':f'ornament{i}','kind':'ornament','runs':[]})
    return {'schema_version':1,'book_id':'test-only','revision':'test-only',
            'documents':[{'id':'test','blocks':blocks}],'pieces':pieces,
            'chapters':[{'id':f'c{i}','title':c['title'],'start_piece':f'p{c["start_word"]}',
                         'end_piece_exclusive':None if c['end_word'] is None else f'p{c["end_word"]}'} for i,c in enumerate(expectation['chapters'])],
            'notes':[{'id':f'n{i}','text':text} for i,text in enumerate(expectation['note_texts'])]}


class ValidatorTests(unittest.TestCase):
    def test_handwritten_spans_resolve_to_expected_words(self):
        for name,case in CASES.items():
            if 'words' not in case['expected']:
                continue
            nodes = source_nodes(case)
            for word in case['expected']['words']:
                actual = ''.join(nodes[(s['document'],s['node'])][s['offset']:s['offset']+s['length']] for s in word['sources'])
                self.assertEqual(word['text'],actual,(name,word))

    def test_checker_accepts_oracle_examples(self):
        for name in ('inline_unicode_hidden','logical_chapters','images_unsupported'):
            self.assertEqual(validate_export(name,oracle_book(name))['status'],'pass')

    def test_rejects_mutations(self):
        mutations = [
            lambda b: b['pieces'][1].update(text='un'),
            lambda b: b['pieces'][4]['sources'][0].update(offset=11),
            lambda b: b['chapters'][0].update(end_piece_exclusive=None),
            lambda b: b['pieces'][0].update(block_id='absent'),
            lambda b: b['pieces'][0]['sources'][0].update(length=-1),
            lambda b: b['pieces'][0]['sources'][0].update(length=True),
            lambda b: b['pieces'][2].update(word_id='w0'),
            lambda b: b['notes'][0].update(text='Wrong note.'),
            lambda b: b['documents'][0]['blocks'][1]['runs'][1].update(emphasis=False),
            lambda b: b['documents'][0]['blocks'][0]['runs'].clear(),
            lambda b: b['pieces'][-2].update(note_id='missing'),
            lambda b: b['pieces'][-2].update(sources=[]),
            lambda b: (b['notes'][0].pop('id'),b['pieces'][-2].pop('note_id')),
        ]
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                book = oracle_book('inline_unicode_hidden')
                mutation(book)
                with self.assertRaises(Invalid):
                    validate_export('inline_unicode_hidden',book)

    def test_split_display_pieces_keep_lexical_word_and_sources(self):
        book = oracle_book('inline_unicode_hidden')
        original = book['pieces'][1]
        left = dict(original,text='un',sources=original['sources'][:1])
        right = dict(original,id='p1b',text='believable',sources=original['sources'][1:])
        book['pieces'][1:2] = [left,right]
        self.assertEqual(validate_export('inline_unicode_hidden',book)['status'],'pass')

    def test_explicit_node_map(self):
        book = oracle_book('logical_chapters')
        mapping = {}
        for item in book['pieces']+[r for b in book['documents'][0]['blocks'] for r in b['runs']]:
            for source in item['sources']:
                # Oracle pieces/runs share spans; avoid applying twice.
                if not source['node'].startswith('external:'):
                    old = source['node']; source['node']='external:'+old
                    mapping[source['document']+'#'+source['node']] = old
        with self.assertRaises(Invalid):
            validate_export('logical_chapters',book)
        self.assertEqual(validate_export('logical_chapters',book,mapping)['status'],'pass')

    def test_restore_decisions(self):
        actual = [dict(case['expected'],id=case['id']) for case in RESTORE]
        self.assertEqual(validate_restore(actual)['restore_cases'],6)
        actual[2]['action']='offer_passage'
        with self.assertRaises(Invalid):
            validate_restore(actual)

    def test_fixture_archives_reproducible(self):
        with tempfile.TemporaryDirectory() as a, tempfile.TemporaryDirectory() as b:
            generate(a); generate(b)
            for path in Path(a).glob('*.epub'):
                self.assertEqual(path.read_bytes(),(Path(b)/path.name).read_bytes())
            with zipfile.ZipFile(Path(a)/'inline_unicode_hidden.epub') as archive:
                self.assertEqual(archive.namelist()[0],'mimetype')
                self.assertEqual(archive.getinfo('mimetype').compress_type,zipfile.ZIP_STORED)
                self.assertEqual(archive.read('mimetype'),b'application/epub+zip')


if __name__ == '__main__':
    unittest.main()
