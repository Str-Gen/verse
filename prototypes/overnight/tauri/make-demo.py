#!/usr/bin/env python3
"""Generate original, redistributable content with scalar source addresses."""
import json
from pathlib import Path
root = Path(__file__).parent
book = dict(schema_version=1, book_id='overnight-tauri-synthetic', revision='synthetic-v1', title='The observatory garden', documents=[], pieces=[], chapters=[], notes=[])
doc = dict(id='garden.xhtml', blocks=[])
book['documents'].append(doc)
def block(kind, texts, **extra):
    bid = f'b{len(doc["blocks"])}'
    runs=[]
    for ri, (text, emphasis, strong) in enumerate(texts):
        node=f'0/{len(doc["blocks"])}/{ri}/text'
        sources=[dict(document=doc['id'],node=node,offset=0,length=len(text))]
        runs.append(dict(text=text,emphasis=emphasis,strong=strong,sources=sources))
    b=dict(id=bid,kind=kind,runs=runs,**extra)
    doc['blocks'].append(b)
    # Join words across inline runs, retaining every contributing source range.
    chars=[]
    for run in runs:
        for offset, char in enumerate(run['text']): chars.append((char,run['sources'][0]['node'],offset))
    groups=[]; active=[]
    for item in chars:
        if item[0].isspace():
            if active: groups.append(active); active=[]
        else: active.append(item)
    if active: groups.append(active)
    for gi, group in enumerate(groups):
        sources=[]
        for char,node,offset in group:
            if sources and sources[-1]['node']==node: sources[-1]['length']+=1
            else: sources.append(dict(document=doc['id'],node=node,offset=offset,length=1))
        pid=f'p{len(book["pieces"])}'
        book['pieces'].append(dict(id=pid,text=''.join(c for c,_,_ in group),word_id=pid,sources=sources,block_id=bid,paragraph_end=gi==len(groups)-1))
    if not groups:
        pid=f'p{len(book["pieces"])}'
        book['pieces'].append(dict(id=pid,text=extra.get('alt',kind),word_id=pid,sources=[],block_id=bid,kind=kind,paragraph_end=True))
    return book['pieces'][-len(groups)]['id'] if groups else pid
block('heading', [('The observatory garden',False,False)],level=1)
block('paragraph',[('An original synthetic book for testing. Select any passage, press Space, then Escape to return. No private publication text is included.',False,False)])
start=block('heading',[('A morning of small discoveries',False,False)],level=2)
block('paragraph',[('An 🦉 watched the un',False,False),('believ',True,False),('able sunrise. Mira called it ',False,False),('a patient experiment',False,True),('. Every word belonged to the same quiet morning.',False,False)])
for i in range(18):
    block('paragraph',[(f'On garden walk {i+1}, Mira stopped beside the brass telescope. The path curved around a pool of rainwater, and the city sounded distant beyond the wall. She recorded the shape of a leaf, then waited for the wind to turn it. There was no prize for finishing first. Reading the notes slowly revealed what yesterday had hidden.',False,False)])
block('list_item',[('Observe the light before adjusting the lens.',False,False)])
block('list_item',[('Keep a record of what changed.',False,False)])
block('image',[],alt='Synthetic garden at dawn',resource='synthetic-landscape.svg')
block('paragraph',[('After the picture, the path continued past the gate.',False,False)])
second=block('heading',[('The evening notebook',False,False)],level=2)
book['chapters'].append(dict(id='morning',title='A morning of small discoveries',start_piece=start,end_piece_exclusive=second))
block('ornament',[],alt='Decorative divider')
block('paragraph',[('At dusk, Mira compared the new pages with the old notebook. She had counted fewer things and noticed more. The telescope remained by the window, ready for another clear night.',False,False)])
block('note',[('A synthetic note: the observatory and its garden are imaginary.',False,False)],note_id='note1')
block('unsupported',[],alt='Synthetic table: rendering is not supported')
block('paragraph',[('The final sentence remains available after unsupported content. The notebook closes here.',False,False)])
book['chapters'].append(dict(id='evening',title='The evening notebook',start_piece=second,end_piece_exclusive=None))
(root/'web/demo.json').write_text(json.dumps(book,ensure_ascii=False,indent=2)+'\n')
