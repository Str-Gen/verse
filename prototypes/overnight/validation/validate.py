#!/usr/bin/env python3
"""Validate version-1 exports against independent synthetic expectations."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import xml.etree.ElementTree as ET

from fixtures import CASES, RESTORE, generate


class Invalid(ValueError):
    pass


def require(ok, message):
    if not ok:
        raise Invalid(message)


def source_nodes(case):
    result = {}
    def visit(doc, element, path):
        result[(doc,path)] = ''  # Zero-length source for non-text events.
        if element.text is not None:
            result[(doc,path+'/text')] = element.text
        if element.tail is not None:
            result[(doc,path+'/tail')] = element.tail
        for i, child in enumerate(element):
            visit(doc,child,f'{path}/{i}')
    for doc, xml in case['documents'].items():
        if xml is not None:
            visit(doc,ET.fromstring(xml),'0')
    return result


def merged(spans):
    output = []
    for s in spans:
        s = {k:s[k] for k in ('document','node','offset','length')}
        if output and all(output[-1][k] == s[k] for k in ('document','node')) and output[-1]['offset']+output[-1]['length'] == s['offset']:
            output[-1]['length'] += s['length']
        else:
            output.append(s)
    return output


def validate_export(case_name, book, node_map=None):
    require(case_name in CASES,'unknown fixture')
    expectation = CASES[case_name]['expected']
    require(book.get('schema_version') == 1,'schema_version must be 1')
    require(isinstance(book.get('book_id'),str) and book['book_id'],'missing book identity')
    require(isinstance(book.get('revision'),str) and book['revision'],'missing revision identity')
    for key in ('documents','pieces','chapters','notes'):
        require(isinstance(book.get(key),list),f'{key} must be an array')
    if expectation.get('failure_or_placeholder'):
        require(any(p.get('kind') == 'unsupported' and p.get('text') for p in book['pieces']),
                'malformed input must fail explicitly or expose an unsupported placeholder')
        return {'case':case_name,'status':'pass','checks':['explicit malformed-input placeholder']}
    nodes = source_nodes(CASES[case_name])
    node_map = node_map or {}

    def checked_sources(obj, text=None):
        spans = obj.get('sources')
        require(isinstance(spans,list) and len(spans)>0,'missing source spans')
        resolved = []
        reconstructed = ''
        for s in spans:
            require(isinstance(s,dict),'source must be an object')
            require(all(k in s for k in ('document','node','offset','length')),'incomplete source span')
            require(type(s['offset']) is int and type(s['length']) is int,'source offset/length must be integers')
            require(s['offset'] >= 0 and s['length'] >= 0,'negative source span')
            mapped = node_map.get(s['document']+'#'+s['node'],s['node'])
            key = (s['document'],mapped)
            require(key in nodes,f'unknown source node {key}; provide an audited node map if path conventions differ')
            value = nodes[key]
            require(s['offset']+s['length'] <= len(value),'source range exceeds decoded Unicode scalar length')
            reconstructed += value[s['offset']:s['offset']+s['length']]
            resolved.append(dict(s,node=mapped))
        if text is not None:
            require(reconstructed == text,'source span text differs from exported text')
        return resolved

    blocks = [b for d in book['documents'] for b in d.get('blocks',[])]
    block_ids = [b.get('id') for b in blocks]
    require(all(isinstance(i,str) and i for i in block_ids) and len(set(block_ids))==len(block_ids),'block IDs must be nonempty and unique')
    blocks_by_id = {b['id']:b for b in blocks}
    note_ids = [n.get('id') for n in book['notes']]
    require(all(isinstance(i,str) and i for i in note_ids) and len(set(note_ids))==len(note_ids),'note IDs must be nonempty and unique')
    piece_ids = [p.get('id') for p in book['pieces']]
    require(all(isinstance(i,str) and i for i in piece_ids) and len(set(piece_ids))==len(piece_ids),'piece IDs must be nonempty and unique')
    groups = []
    finished_word_ids = set()
    positions = {}
    kinds = Counter()
    events = []
    word_starts = set()
    for p in book['pieces']:
        kind = p.get('kind','word')
        kinds[kind] += 1
        require(p.get('block_id') in block_ids,'piece references unknown block')
        positions[p['id']] = len(groups)
        if kind != 'word':
            require(kind in ('image','unsupported','note'),'unknown playback event kind')
            block = blocks_by_id[p['block_id']]
            require(block.get('kind')==kind,'playback event must reference its conventional block kind')
            if kind=='image':
                require(isinstance(block.get('resource'),str) and block['resource'] and not block['resource'].startswith(('http:','https:')),'image block must retain a local resource')
                require(isinstance(block.get('alt'),str),'image block must retain alt text')
            spans = checked_sources(p)
            require(len(spans)==1 and spans[0]['offset']==0 and spans[0]['length']==0,'event must address its source element')
            events.append({'kind':kind,'after_word':len(groups),'document':spans[0]['document'],'node':spans[0]['node']})
            if kind == 'note':
                require(isinstance(p.get('note_id'),str) and p['note_id'] in note_ids,'note event has unknown note_id')
            continue
        text = p.get('text')
        require(isinstance(text,str) and bool(text),'word piece must contain text')
        spans = checked_sources(p,text)
        wid = p.get('word_id')
        require(isinstance(wid,str) and wid,'word piece must have word_id')
        if groups and groups[-1]['word_id'] == wid:
            positions[p['id']] = len(groups)-1
            groups[-1]['text'] += text
            groups[-1]['sources'] += spans
        else:
            require(wid not in finished_word_ids,'word_id reused after another word')
            finished_word_ids.add(wid)
            word_starts.add(p['id'])
            groups.append({'word_id':wid,'text':text,'sources':spans})
    expected_words = expectation.get('words')
    wanted = [w['text'] for w in expected_words] if expected_words is not None else expectation['visible_words']
    require([g['text'] for g in groups] == wanted,'visible lexical words differ from independent fixture expectation')
    if expected_words is not None:
        for actual, expected in zip(groups,expected_words):
            require(merged(actual['sources']) == merged(expected['sources']),f'source spans differ for lexical word {groups.index(actual)}')
    actual_chapters = []
    for c in book['chapters']:
        require(c.get('start_piece') in positions,'chapter starts at an unknown piece')
        require(c['start_piece'] in word_starts,'chapter must start at the first piece of its lexical word')
        end = c.get('end_piece_exclusive')
        require(end is None or end in positions,'chapter ends at an unknown piece')
        require(end is None or end in word_starts,'chapter end must be a lexical-word boundary')
        actual_chapters.append({'title':c.get('title'),'start_word':positions[c['start_piece']],
                                'end_word':None if end is None else positions[end]})
    require(actual_chapters == expectation['chapters'],'logical chapter ranges differ')
    conventional = ''
    emphasized = ''
    for b in blocks:
        for r in b.get('runs',[]):
            require(isinstance(r.get('text'),str),'run text must be a string')
            checked_sources(r,r['text'])
            if b.get('kind') != 'note':
                conventional += r['text']+'\n'
            if r.get('emphasis'):
                emphasized += r['text']
    for forbidden in expectation.get('forbidden_text',[]):
        require(forbidden not in conventional,'hidden text leaked into conventional view')
    for required in expectation.get('required_emphasis',[]):
        require(required in emphasized,'inline emphasis lost')
    # A visible block must retain each word, including joined inline runs. Separating
    # blocks is intentional; source spans above preserve spaces inside each run.
    rendered_words = []
    for b in blocks:
        if b.get('kind') in ('heading','paragraph','list_item'):
            rendered_words += ''.join(r['text'] for r in b.get('runs',[])).split()
    require(rendered_words == wanted,'conventional words differ from playback words')
    require(events == expectation.get('events',[]),'playback event order/source position differs')
    for field, kind in [('note_events','note'),('image_events','image'),('unsupported_events','unsupported')]:
        if field in expectation:
            require(kinds[kind] == expectation[field],f'{kind} playback event count differs')
    if 'ornament_blocks' in expectation:
        require(sum(b.get('kind')=='ornament' for b in blocks)==expectation['ornament_blocks'],'ornaments must remain conventional blocks and skip playback')
    require(len(book['notes']) == len(expectation['note_texts']),'note count differs')
    # notes has no fully specified nested schema in CONTRACT.md. Accept the
    # documented common shapes; other adapters must emit text explicitly.
    for note, expected in zip(book['notes'],expectation['note_texts']):
        text = note.get('text')
        if text is None:
            text = ' '.join(''.join(r['text'] for r in b.get('runs',[])) for b in note.get('blocks',[]))
        require(' '.join(text.split()) == expected,'note text differs; adapter may expose note.text')
    return {'case':case_name,'status':'pass','words':len(groups),'chapters':len(actual_chapters),
            'checks':['word order','scalar source spans','logical chapters','conventional/playback agreement','events','notes']}


def validate_restore(actual):
    require(isinstance(actual,list),'restore results must be an array')
    require(len(actual)==len(RESTORE),'restore case count differs')
    indexed = {r.get('id'):r for r in actual}
    require(len(indexed)==len(RESTORE),'duplicate restore case IDs')
    for case in RESTORE:
        require(case['id'] in indexed,f'missing restore case {case["id"]}')
        result = indexed[case['id']]
        for key,value in case['expected'].items():
            require(result.get(key)==value,f'restore {case["id"]}: {key} differs')
    return {'status':'pass','restore_cases':len(RESTORE)}


def run_core(command, directory, timeout):
    directory = Path(directory)
    fixture_dir = generate(directory/'fixtures')
    output_dir = directory/'exports'
    output_dir.mkdir(exist_ok=True)
    results = []
    books = {}
    for name in [*CASES,'not_a_zip']:
        target = output_dir/f'{name}.json'
        target.unlink(missing_ok=True)
        invocation = [*command,'export','--epub',str((fixture_dir/f'{name}.epub').resolve()),'--output',str(target.resolve())]
        try:
            completed = subprocess.run(invocation,capture_output=True,text=True,timeout=timeout)
            malformed = name == 'not_a_zip' or CASES[name]['expected'].get('failure_or_placeholder')
            if completed.returncode:
                require(malformed and completed.returncode > 0 and bool(completed.stderr.strip() or completed.stdout.strip()),
                        'export failed without an accepted diagnostic failure')
                require(not target.exists(),'failed import left an apparently valid output file')
                result = {'case':name,'status':'pass','checks':['explicit rejection'],'exit_code':completed.returncode}
            else:
                require(name != 'not_a_zip','invalid ZIP must be rejected')
                book = json.loads(target.read_text())
                result = validate_export(name,book)
                books[name] = book
        except (Invalid,OSError,ValueError,KeyError,TypeError,AttributeError,IndexError,subprocess.TimeoutExpired) as error:
            result = {'case':name,'status':'fail','reason':str(error)}
        results.append(result)
    if 'edition_original' in books:
        target = output_dir/'edition_original_repeat.json'
        target.unlink(missing_ok=True)
        try:
            repeated = subprocess.run([*command,'export','--epub',str((fixture_dir/'edition_original.epub').resolve()),'--output',str(target.resolve())],capture_output=True,text=True,timeout=timeout)
            require(repeated.returncode==0,'repeat export failed')
            repeat_book=json.loads(target.read_text())
            require(repeat_book.get('revision')==books['edition_original']['revision'],'same bytes produced a different revision')
            results.append({'case':'stable_revision','status':'pass'})
        except (Invalid,OSError,ValueError,KeyError,TypeError,AttributeError,IndexError,subprocess.TimeoutExpired) as error:
            results.append({'case':'stable_revision','status':'fail','reason':str(error)})
    # Revision must change with changed content. Local identity semantics require
    # an identity/store adapter and are not guessed from publisher identifiers.
    editions = [books[n] for n in CASES if n.startswith('edition_') and n in books]
    if len(editions) == 5:
        results.append({'case':'edition_revisions','status':'pass' if len({b['revision'] for b in editions})==5 else 'fail'})
    summary = {'command':command,'results':results,'pending':['restore policy adapter','native frontend evidence','per-book identity and persistence'],
               'fixture_sha256':{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in fixture_dir.glob('*.epub')}}
    (directory/'results.json').write_text(json.dumps(summary,indent=2)+'\n')
    print(json.dumps(summary,indent=2))
    return 1 if any(r['status']=='fail' for r in results) else 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='operation',required=True)
    gen = sub.add_parser('fixtures'); gen.add_argument('--output',type=Path,required=True)
    val = sub.add_parser('validate'); val.add_argument('--case',choices=CASES,required=True); val.add_argument('--book-json',type=Path,required=True); val.add_argument('--node-map',type=Path)
    restore = sub.add_parser('restore'); restore.add_argument('--results',type=Path,required=True)
    restore_run = sub.add_parser('run-restore'); restore_run.add_argument('--output',type=Path,required=True); restore_run.add_argument('--timeout',type=float,default=30); restore_run.add_argument('command',nargs=argparse.REMAINDER)
    run = sub.add_parser('run-core'); run.add_argument('--output',type=Path,required=True); run.add_argument('--timeout',type=float,default=30); run.add_argument('command',nargs=argparse.REMAINDER)
    args = parser.parse_args()
    try:
        if args.operation == 'fixtures':
            generate(args.output); print(f'Generated {len(CASES)+1} synthetic EPUB cases in {args.output}')
        elif args.operation == 'validate':
            mapping = json.loads(args.node_map.read_text()) if args.node_map else None
            print(json.dumps(validate_export(args.case,json.loads(args.book_json.read_text()),mapping),indent=2))
        elif args.operation == 'restore':
            print(json.dumps(validate_restore(json.loads(args.results.read_text())),indent=2))
        elif args.operation == 'run-restore':
            command = args.command[1:] if args.command[:1] == ['--'] else args.command
            require(bool(command),'provide restore adapter command after --')
            requests = [{k:v for k,v in case.items() if k != 'expected'} for case in RESTORE]
            completed = subprocess.run(command,input=json.dumps(requests),capture_output=True,text=True,timeout=args.timeout)
            require(completed.returncode==0,'restore adapter failed')
            results = json.loads(completed.stdout)
            args.output.parent.mkdir(parents=True,exist_ok=True)
            args.output.write_text(json.dumps(results,indent=2)+'\n')
            print(json.dumps(validate_restore(results),indent=2))
        else:
            command = args.command[1:] if args.command[:1] == ['--'] else args.command
            require(bool(command),'provide core command after --')
            return run_core(command,args.output,args.timeout)
    except (Invalid,OSError,ValueError,KeyError,TypeError,AttributeError,IndexError,subprocess.TimeoutExpired) as error:
        print(f'FAIL: {error}',file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
