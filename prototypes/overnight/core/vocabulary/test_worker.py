import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
import unittest

from worker import document
from vocabulary import VocabularyCapture
from dictionary import build, lookup


def book():
    text = 'An eight-hundred-year-old tree. Whorls remain.'
    spans = lambda start, length: [{'document':'d', 'node':'n', 'offset':start, 'length':length}]
    tokens = ['An','eight-','hundred-','year-','old','tree.','Whorls','remain.']
    pieces = []
    offset = 0
    for i, token in enumerate(tokens):
        offset = text.index(token, offset)
        pieces.append(dict(id=str(i), block_id='b', kind='word', text=token, sources=spans(offset,len(token))))
        offset += len(token)
    return dict(title='Synthetic', pieces=pieces, documents=[{'blocks':[{'id':'b','runs':[{'text':text,'sources':spans(0,len(text))}]}]}])


class VocabularyTests(unittest.TestCase):
    def test_source_mapping_compound_capture_and_feedback_survive_restart(self):
        paragraphs, pieces, indices = document(book())
        self.assertEqual(indices[4], 4)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)/'words.sqlite'
            notebook = VocabularyCapture(path, None)
            doc = notebook.open_document(paragraphs,pieces,'Synthetic')
            self.assertNotEqual(notebook.open_document(paragraphs,pieces,'Synthetic','one'), notebook.open_document(paragraphs,pieces,'Synthetic','two'))
            partial = notebook.capture(doc,[dict(piece=i,start=i*100,end=(i+1)*100) for i in range(3)],300,'seconds','partial')
            self.assertNotIn('eight-hundred-year-old',[c['word'] for c in partial['candidates']])
            full = notebook.capture(doc,[dict(piece=i,start=i*100,end=(i+1)*100) for i in range(7)],700,'seconds','full')
            words = [c['word'] for c in full['candidates']]
            self.assertIn('eight-hundred-year-old',words)
            candidate = words.index('Whorls')
            notebook.feedback(full['id'],candidate,'recognition')
            self.assertEqual(notebook.review(doc)['words'][0]['candidate']['sentence'],'Whorls remain.')
            notebook.close()
            notebook = VocabularyCapture(path,None)
            doc = notebook.open_document(paragraphs,pieces,'Synthetic')
            self.assertEqual(notebook.review(doc)['words'][0]['reasons'],['recognition'])
            self.assertEqual(notebook.capture(doc,[dict(piece=0,start=0,end=10)],10,'seconds','full')['id'],full['id'])
            notebook.close()

    def test_window_replacement_keeps_saved_sentence_and_book_review(self):
        with tempfile.TemporaryDirectory() as directory:
            notebook = VocabularyCapture(Path(directory)/'words.sqlite', None)
            paragraphs, pieces, _ = document(book())
            old = notebook.open_document(paragraphs, pieces, 'Synthetic', 'same-book', replace=True)
            captured = notebook.capture(old, [dict(piece=6, start=0, end=100)], 100, 'seconds', 'old-window')
            notebook.feedback(captured['id'], 0, 'recognition')
            new = notebook.open_document(['Next passage.'], [dict(paragraph=0,start=0,end=4,sentence=0)], 'Synthetic', 'same-book', replace=True)
            with self.assertRaises(KeyError):
                notebook.capture(old, [dict(piece=6,start=0,end=100)],100,'seconds')
            review = notebook.review(new)
            self.assertEqual(review['words'][0]['candidate']['sentence'], 'Whorls remain.')
            self.assertEqual(review['captures'][0]['id'], captured['id'])
            notebook.close()

    def test_nearby_taps_nominate_distinct_ranked_words(self):
        class Judge:
            def rank(self, capture, confirmed):
                return {'judgments':[{'candidate':i,'score':4-i} for i in range(len(capture['candidates']))]}
        with tempfile.TemporaryDirectory() as directory:
            notebook=VocabularyCapture(Path(directory)/'words.sqlite',Judge())
            paragraphs,pieces,_=document(book())
            doc=notebook.open_document(paragraphs,pieces,'Synthetic')
            exposures=[dict(piece=i,start=i*100,end=(i+1)*100) for i in range(8)]
            first=notebook.capture(doc,exposures,800,'sentences','first')
            second=notebook.capture(doc,exposures,800,'sentences','second')
            for _ in range(200):
                review=notebook.review(doc)
                if all(c['status']=='ranked' for c in review['captures']): break
                time.sleep(.005)
            self.assertEqual(len(review['suggestions']),2)
            self.assertNotEqual(review['suggestions'][0]['candidate']['word'],review['suggestions'][1]['candidate']['word'])
            self.assertEqual(len(review['words']),0)
            self.assertNotEqual(first['id'],second['id'])
            notebook.close()

    def test_worker_protocol_offline_capture_definition_and_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            process = subprocess.Popen(['python3',str(Path(__file__).with_name('worker.py')),directory],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True,
                env={**os.environ,'TYPESAFE_API_KEY':'','VERSE_KEY_FILE':directory+'/absent','VERSE_DICTIONARY':directory+'/absent'})
            def request(**data):
                process.stdin.write(json.dumps(data)+'\n'); process.stdin.flush()
                return json.loads(process.stdout.readline())
            try:
                self.assertIn('result',request(action='open',book=book()))
                self.assertIn('error',request(action='capture',exposures=[],now=0,scope='seconds',tap_id='empty'))
                captured = request(action='capture',exposures=[dict(piece=0,start=0,end=100)],now=100,scope='seconds',tap_id='tap')['result']
                self.assertEqual(captured['status'],'awaiting_key')
                self.assertEqual(len(request(action='review')['result']['captures']),1)
                self.assertFalse(request(action='definition',word='whorls')['result']['available'])
            finally:
                process.stdin.close(); process.wait(timeout=5); process.stdout.close()
            self.assertEqual((Path(directory)/'vocabulary.sqlite').stat().st_mode & 0o777,0o600)

    def test_local_dictionary_preserves_senses_and_distinguishes_missing(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory); source=path/'source.jsonl'; database=path/'dictionary.sqlite'
            source.write_text(json.dumps(dict(lang_code='en',word='whorl',pos='noun',senses=[{'glosses':['A coil.']},{'glosses':['A circular arrangement.'],'tags':['botany']}]))+'\n')
            build(source,database)
            self.assertEqual(len(lookup(database,'Whorl')['entries'][0]['senses']),2)
            self.assertEqual(lookup(database,'missing')['entries'],[])
            self.assertTrue(lookup(database,'missing')['available'])


if __name__ == '__main__': unittest.main()
