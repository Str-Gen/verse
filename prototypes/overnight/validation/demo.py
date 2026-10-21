#!/usr/bin/env python3
"""Create a long, original schema-v1 book. This is not an EPUB importer."""
import argparse
import hashlib
import json
from pathlib import Path


def build_book():
    blocks, pieces = [], []
    def add(text, kind, node):
        block_id = f'b{len(blocks)}'
        span = {'document':'synthetic.xhtml','node':node,'offset':0,'length':len(text)}
        blocks.append({'id':block_id,'kind':kind,'runs':[{'text':text,'emphasis':False,'strong':False,'sources':[span]}]})
        offset = 0
        for token in text.split(' '):
            i = len(pieces)
            pieces.append({'id':f'p{i}','word_id':f'w{i}','kind':'word','text':token,'block_id':block_id,
                           'sources':[dict(span,offset=offset,length=len(token))], 'paragraph_end':offset+len(token)==len(text)})
            offset += len(token)+1
    chapters = []
    for chapter in range(2):
        start = f'p{len(pieces)}'
        if chapters:
            chapters[-1]['end_piece_exclusive'] = start
        chapters.append({'id':f'c{chapter}','title':['River','Ridge'][chapter],'start_piece':start,'end_piece_exclusive':None})
        add(chapters[-1]['title'],'heading',f'0/1/{len(blocks)}/text')
        for paragraph in range(40):
            serial = chapter*40+paragraph
            add(f'Passage{serial:03d} carries café A😀B across the quiet valley. Every source stays stable when the page changes.',
                'paragraph',f'0/1/{len(blocks)}/text')
    revision = hashlib.sha256(json.dumps(blocks,sort_keys=True).encode()).hexdigest()
    return {'schema_version':1,'book_id':'validation-original-synthetic','revision':revision,
            'title':'Synthetic pagination and passage return','documents':[{'id':'synthetic.xhtml','blocks':blocks}],
            'pieces':pieces,'chapters':chapters,'notes':[]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,default=Path('.scratch/demo.json'))
    args = parser.parse_args()
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps(build_book(),ensure_ascii=False,indent=2)+'\n')
    print(f'{args.output}: synthetic book, 2 chapters, 80 paragraphs')


if __name__ == '__main__':
    main()
