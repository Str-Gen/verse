#!/usr/bin/env python3
"""Drive an external native UI adapter or check its observation trace.

An adapter executes actions against the real app. This program does not render
or simulate a frontend. Native provenance remains an integration obligation.
"""
import argparse
import json
from pathlib import Path
import subprocess
import sys

from offsets import VECTORS
from measure import stop
from validate import Invalid, require


def plan(book):
    words = [p for p in book['pieces'] if p.get('kind','word')=='word']
    target = next((i for i,p in enumerate(words) if p['text']=='Passage012'),None)
    require(target is not None and target+3<len(words),'use demo.py synthetic pagination book')
    selected = words[target]
    last = words[target+3]
    return {'version':1,'fixture_revision':book['revision'],'synthetic_only':True,
            'unicode_vectors':VECTORS,
            'actions':[
                {'id':'launch','action':'launch','viewport':[960,720],'fresh_storage':True},
                {'id':'right','action':'key','key':'ArrowRight'},
                {'id':'left','action':'key','key':'ArrowLeft'},
                {'id':'select','action':'navigate_and_select_source','sources':selected['sources']},
                {'id':'play','action':'start_playback'},
                {'id':'stop','action':'advance_then_stop','additional_words':3,'last_sources':last['sources']},
                {'id':'return','action':'return_to_conventional'},
                {'id':'resize','action':'resize','viewport':[640,480]},
                {'id':'speed','action':'set_speed','wpm':425},
                {'id':'restart','action':'restart_same_book_and_storage'},
            ]}


def check(book, trace):
    requested = plan(book)
    require(trace.get('version')==1,'unsupported trace version')
    require(trace.get('fixture_revision')==book['revision'],'trace has wrong fixture revision')
    provenance = trace.get('provenance',{})
    require(provenance.get('runtime') in ('qt-native','tauri-native'),'browser-only traces do not verify a native frontend')
    for key in ('commit','executable_sha256','driver','platform','storage','capture_directory'):
        require(isinstance(provenance.get(key),str) and provenance[key],f'missing provenance {key}')
    require(len(provenance['executable_sha256'])==64,'record executable SHA-256')
    events = trace.get('observations',[])
    require([e.get('id') for e in events]==[a['id'] for a in requested['actions']],'missing or reordered UI observations')
    indexed = {e['id']:e for e in events}
    pieces = {p['id']:p for p in book['pieces']}
    order = {p['id']:i for i,p in enumerate(book['pieces'])}
    for event in events:
        require(event.get('errors')==[],'driver or application reported errors')
        visible = event.get('visible_piece_ids',[])
        require(all(p in pieces for p in visible),'unknown visible piece')
        require([order[p] for p in visible]==sorted(set(order[p] for p in visible)),'visible pieces must be unique and in source order')
        if visible:
            require([order[p] for p in visible]==list(range(order[visible[0]],order[visible[-1]]+1)),'page has missing interior source pieces')
        require(isinstance(event.get('capture'),str) and event['capture'],'each step needs a screenshot or native accessibility capture path')
    for ident in ('launch','right','left','select','return','resize','speed','restart'):
        event = indexed[ident]
        require(event.get('mode')=='conventional' and event.get('playing') is False,f'{ident}: conventional view must be stopped')
        require(bool(event.get('visible_piece_ids')),f'{ident}: no visible source pieces')
    first = indexed['launch']['visible_piece_ids']
    right = indexed['right']['visible_piece_ids']
    require(order[right[0]]>order[first[0]],'right arrow did not advance the page')
    require(order[right[0]]<=order[first[-1]]+1,'right arrow skipped source pieces between pages')
    require(indexed['left']['visible_piece_ids']==first,'left arrow did not restore the first page')
    require(indexed['launch'].get('wpm')==350,'fresh book must start at 350 WPM')
    selected = requested['actions'][3]['sources']
    last = requested['actions'][5]['last_sources']
    require(indexed['select'].get('selected_sources')==selected,'selection did not retain source identity')
    require(indexed['play'].get('displayed_sources')==selected,'playback did not begin at the selected passage')
    require(indexed['play'].get('mode')=='playback' and indexed['play'].get('playing') is True,'playback did not start')
    require(indexed['stop'].get('displayed_sources')==last and indexed['stop'].get('playing') is False,'last displayed passage differs or playback is still running')
    last_id = next(p['id'] for p in book['pieces'] if p.get('sources')==last)
    for ident in ('return','resize','speed','restart'):
        event = indexed[ident]
        require(last_id in event['visible_piece_ids'],f'{ident}: last displayed passage is outside visible page')
        require(event.get('selected_sources')==last,f'{ident}: saved source position changed')
    require(indexed['launch'].get('viewport')==[960,720] and indexed['resize'].get('viewport')==[640,480], 'requested reflow dimensions were not observed')
    require(indexed['speed'].get('wpm')==425 and indexed['restart'].get('wpm')==425,'saved per-book speed not restored')
    actual_vectors = trace.get('unicode_results')
    require(actual_vectors==[{'text':v['text'],'utf16_boundaries':v['utf16_boundaries'],
                              'scalar_boundaries':v['scalar_boundaries'],'rejected_utf16':v['invalid_utf16']} for v in VECTORS],
            'frontend scalar/UTF-16 boundary conversions differ')
    return {'status':'pass','checks':['page arrows','selection to playback','last displayed passage return','resize','stopped restart','saved speed','UTF-16 conversion'],
            'evidence_boundary':'Trace assertions passed. Inspect native capture files and driver implementation independently; this checker cannot authenticate observations.'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='operation',required=True)
    for name in ('plan','check','run'):
        p = sub.add_parser(name); p.add_argument('--book-json',type=Path,required=True)
        if name=='check': p.add_argument('--trace',type=Path,required=True)
        else: p.add_argument('--output',type=Path,required=True)
        if name=='run':
            p.add_argument('--timeout',type=float,default=120)
            p.add_argument('driver',nargs=argparse.REMAINDER)
    args=parser.parse_args()
    try:
        book=json.loads(args.book_json.read_text())
        if args.operation=='check':
            print(json.dumps(check(book,json.loads(args.trace.read_text())),indent=2))
        else:
            request=plan(book)
            request['book_json']=str(args.book_json.resolve())
            args.output.parent.mkdir(parents=True,exist_ok=True)
            if args.operation=='plan':
                args.output.write_text(json.dumps(request,ensure_ascii=False,indent=2)+'\n')
                print(args.output)
            else:
                driver=args.driver[1:] if args.driver[:1]==['--'] else args.driver
                require(bool(driver),'provide an actual native UI driver command after --')
                proc=subprocess.Popen(driver,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,start_new_session=True)
                try:
                    stdout, _ = proc.communicate(json.dumps(request),timeout=args.timeout)
                    code=proc.returncode
                finally:
                    stop(proc)
                require(code==0,'native UI driver failed')
                trace=json.loads(stdout)
                # Keep failing evidence too; all input is synthetic by contract.
                args.output.write_text(json.dumps(trace,ensure_ascii=False,indent=2)+'\n')
                print(json.dumps(check(book,trace),indent=2))
    except (Invalid,OSError,ValueError,KeyError,TypeError,AttributeError,IndexError,subprocess.TimeoutExpired) as error:
        print(f'FAIL: {error}',file=sys.stderr); return 1
    return 0


if __name__=='__main__':
    sys.exit(main())
