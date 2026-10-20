import copy
from pathlib import Path
import sys
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from demo import build_book
from frontend import check,plan
from offsets import VECTORS,scalar_to_utf16,utf16_to_scalar
from validate import Invalid


def oracle_trace(book):
    """Fabricated checker test data, never frontend evidence."""
    request=plan(book)
    selected=request['actions'][3]['sources']; last=request['actions'][5]['last_sources']
    selected_id=next(p['id'] for p in book['pieces'] if p['sources']==selected)
    last_id=next(p['id'] for p in book['pieces'] if p['sources']==last)
    observations=[]
    for action in request['actions']:
        ident=action['id']
        observations.append({'id':ident,'mode':'conventional','playing':False,'wpm':425 if ident in ('speed','restart') else 350,
            'visible_piece_ids':['p0','p1'] if ident in ('launch','left') else ['p2','p3'] if ident=='right' else [p['id'] for p in book['pieces'][int(selected_id[1:]):int(last_id[1:])+1]],
            'selected_sources':last if ident in ('return','resize','speed','restart') else selected,
            'displayed_sources':last if ident=='stop' else selected,
            'viewport':[640,480] if ident=='resize' else [960,720],
            'errors':[],'capture':'unit-test-only.txt'})
    observations[4].update(mode='playback',playing=True)
    return {'version':1,'fixture_revision':book['revision'],'provenance':{'runtime':'qt-native','commit':'unit-test',
            'executable_sha256':'0'*64,'driver':'unit-test-only','platform':'none','storage':'none','capture_directory':'none'},
            'observations':observations,
            'unicode_results':[{'text':v['text'],'utf16_boundaries':v['utf16_boundaries'],'scalar_boundaries':v['scalar_boundaries'],'rejected_utf16':v['invalid_utf16']} for v in VECTORS]}


class FrontendChecks(unittest.TestCase):
    def test_unicode_boundaries(self):
        for vector in VECTORS:
            for scalar,units in zip(vector['scalar_boundaries'],vector['utf16_boundaries']):
                self.assertEqual(scalar_to_utf16(vector['text'],scalar),units)
                self.assertEqual(utf16_to_scalar(vector['text'],units),scalar)
            for offset in vector['invalid_utf16']:
                with self.assertRaises(ValueError):utf16_to_scalar(vector['text'],offset)
        for offset in (-1,99,True):
            with self.assertRaises(ValueError):scalar_to_utf16('A',offset)
            with self.assertRaises(ValueError):utf16_to_scalar('A',offset)

    def test_observation_checker(self):
        book=build_book();trace=oracle_trace(book)
        self.assertEqual(check(book,trace)['status'],'pass')
        mutations=[lambda t:t['observations'][1].update(visible_piece_ids=['p0']),
                   lambda t:t['observations'][2].update(visible_piece_ids=['p1']),
                   lambda t:t['observations'][6].update(selected_sources=[]),
                   lambda t:t['observations'][-1].update(playing=True),
                   lambda t:t['observations'][-1].update(wpm=350),
                   lambda t:t['observations'][7].update(visible_piece_ids=['p0']),
                   lambda t:t['provenance'].update(runtime='browser'),
                   lambda t:t.update(unicode_results=[]),
                   lambda t:t['observations'][1].update(visible_piece_ids=['p20','p21']),
                   lambda t:t['observations'][1].update(visible_piece_ids=['p2','p4'])]
        for mutate in mutations:
            bad=copy.deepcopy(trace);mutate(bad)
            with self.assertRaises(Invalid):check(book,bad)


if __name__=='__main__':unittest.main()
