"""Original synthetic EPUBs and manually specified semantic expectations.

Expected words are never derived by importing/parsing the generated EPUB.
"""
from pathlib import Path
import json
import zipfile
from xml.sax.saxutils import escape

XHTML = 'http://www.w3.org/1999/xhtml'
EPUB = 'http://www.idpf.org/2007/ops'


def document(body, head=''):
    return f'<html xmlns="{XHTML}" xmlns:epub="{EPUB}"><head><title>Fixture</title>{head}</head><body>{body}</body></html>'


# A source expectation uses XML element-child paths rooted at 0, /text or /tail.
# These address decoded text, not bytes in the ZIP or serialized entity spelling.
def word(text, document, *spans):
    return {'text': text, 'sources': [dict(document=document, node=node, offset=offset, length=length)
                                     for node, offset, length in spans]}


def simple(text, doc, node, offset=0):
    return word(text, doc, (node, offset, len(text)))


CASES = {
    'inline_unicode_hidden': {
        'documents': {'text.xhtml': document(
            '<h1 id="one">One</h1><p>un<em>believ</em>able café A😀B 𝄞tone é.</p>'
            '<p hidden="hidden">Hidden attribute.</p><p style="display:none">Hidden style.</p>'
            '<p class="secret">Hidden sheet.</p><div style="visibility:hidden"><p>Hidden ancestor.</p></div>'
            '<p>Visible <span style="display:none">hidden inline </span>tail.</p>'
            '<h1 id="two">Two</h1><p>Read<a epub:type="noteref" href="#n1">1</a> on.</p>'
            '<aside epub:type="footnote" id="n1"><p>A small note.</p></aside>'
            '<script>Publisher script.</script>', '<link rel="stylesheet" href="style.css"/>')},
        'assets': {'style.css': '.secret { display: none; }'},
        'nav': [('One', 'text.xhtml#one'), ('Two', 'text.xhtml#two')],
        'expected': {
            'words': [simple('One','text.xhtml','0/1/0/text'),
                      word('unbelievable','text.xhtml',('0/1/1/text',0,2),('0/1/1/0/text',0,6),('0/1/1/0/tail',0,4)),
                      simple('café','text.xhtml','0/1/1/0/tail',5),
                      simple('A😀B','text.xhtml','0/1/1/0/tail',10),
                      simple('𝄞tone','text.xhtml','0/1/1/0/tail',14),
                      simple('é.','text.xhtml','0/1/1/0/tail',20),
                      simple('Visible','text.xhtml','0/1/6/text'),
                      simple('tail.','text.xhtml','0/1/6/0/tail'),
                      simple('Two','text.xhtml','0/1/7/text'),
                      simple('Read','text.xhtml','0/1/8/text'),
                      simple('on.','text.xhtml','0/1/8/0/tail',1)],
            'chapters': [{'title':'One','start_word':0,'end_word':8}, {'title':'Two','start_word':8,'end_word':None}],
            'note_texts':['A small note.'], 'note_events':1,
            'events':[{'kind':'note','after_word':10,'document':'text.xhtml','node':'0/1/8/0'}],
            'forbidden_text':['Hidden attribute.','Hidden style.','Hidden sheet.','Hidden ancestor.','hidden inline','Publisher script.'],
            'required_emphasis':['believ'],
        },
    },
    'logical_chapters': {
        'documents': {
            'title.xhtml':document('<p>Synthetic atlas</p>'),
            'a.xhtml':document('<h1 id="river">River</h1><p>First reach.</p>'),
            'b.xhtml':document('<p>Across seam.</p><h1 id="ridge">Ridge</h1><p>Last reach.</p>'),
        },
        'nav':[('River','a.xhtml#river'),('Ridge','b.xhtml#ridge')],
        'expected': {
            'words':[simple('Synthetic','title.xhtml','0/1/0/text'),simple('atlas','title.xhtml','0/1/0/text',10),
                     simple('River','a.xhtml','0/1/0/text'),simple('First','a.xhtml','0/1/1/text'),simple('reach.','a.xhtml','0/1/1/text',6),
                     simple('Across','b.xhtml','0/1/0/text'),simple('seam.','b.xhtml','0/1/0/text',7),
                     simple('Ridge','b.xhtml','0/1/1/text'),simple('Last','b.xhtml','0/1/2/text'),simple('reach.','b.xhtml','0/1/2/text',5)],
            'chapters':[{'title':'River','start_word':2,'end_word':7},{'title':'Ridge','start_word':7,'end_word':None}],
            'note_texts':[], 'note_events':0,
        },
    },
    'images_unsupported': {
        'documents':{'art.xhtml':document('<h1 id="art">Art</h1><p>Before.</p>'
            '<img src="ornament.svg" role="presentation" alt=""/>'
            '<img src="landscape.svg" alt="A hill with a trail"/>'
            '<img src="ornament.svg" role="presentation" alt=""/>'
            '<img src="landscape.svg" alt="A hill with a trail"/>'
            '<table><tr><td>Measured terrain</td></tr></table><p>After.</p>')},
        'assets':{
            'ornament.svg':'<svg xmlns="http://www.w3.org/2000/svg" width="80" height="12"><path d="M0 6 H80" stroke="black"/></svg>',
            'landscape.svg':'<svg xmlns="http://www.w3.org/2000/svg" width="320" height="180"><path d="M0 180 L160 20 L320 180Z" fill="green"/></svg>',
        },
        'nav':[('Art','art.xhtml#art')],
        'expected':{'words':[simple('Art','art.xhtml','0/1/0/text'),simple('Before.','art.xhtml','0/1/1/text'),simple('After.','art.xhtml','0/1/7/text')],
                    'chapters':[{'title':'Art','start_word':0,'end_word':None}],
                    'note_texts':[], 'note_events':0, 'image_events':2, 'ornament_blocks':2, 'unsupported_events':1,
                    'events':[{'kind':'image','after_word':2,'document':'art.xhtml','node':'0/1/3'},
                              {'kind':'image','after_word':2,'document':'art.xhtml','node':'0/1/5'},
                              {'kind':'unsupported','after_word':2,'document':'art.xhtml','node':'0/1/6'}]},
    },
    'malformed_xml': {
        'documents': {'bad.xhtml': document('<h1 id="bad">Bad</h1><p>Unclosed')},
        'nav':[('Bad','bad.xhtml#bad')],
        'expected':{'failure_or_placeholder':True},
    },
    'missing_resource': {
        'documents':{'missing.xhtml':None}, 'nav':[('Missing','missing.xhtml')],
        'expected':{'failure_or_placeholder':True},
    },
}

# Editions deliberately share a publisher identifier. Recovery must not rely on it.
for name, paragraph in [('edition_original','The amber lantern rests.'),
                         ('edition_unique','Preface. The amber lantern rests.'),
                         ('edition_ambiguous','The amber lantern rests. The amber lantern rests.'),
                         ('edition_chapter_only','A different passage remains.'),
                         ('edition_no_match','Nothing familiar remains.')]:
    title = 'Elsewhere' if name == 'edition_no_match' else 'Harbor'
    anchor = 'elsewhere' if name == 'edition_no_match' else 'harbor'
    CASES[name] = {'documents':{'edition.xhtml':document(f'<h1 id="{anchor}">{title}</h1><p>{paragraph}</p>')},
                   'nav':[(title,f'edition.xhtml#{anchor}')],
                   'expected':{'visible_words': [title] + paragraph.split(),
                               'chapters':[{'title':title,'start_word':0,'end_word':None}],
                               'note_texts':[], 'note_events':0}}

# Independent input/output examples for a frontend or core restore adapter.
RESTORE = [
    {'id':'same_revision','saved_revision':'old','revision':'old','exact_source_exists':True,
     'passage_candidates':['target'],'chapter_candidates':['harbor'],
     'expected':{'action':'exact','target':'saved','requires_confirmation':False,'preserve_saved':True}},
    {'id':'unique_changed','saved_revision':'old','revision':'new','exact_source_exists':True,
     'passage_candidates':['unique'],'chapter_candidates':['harbor'],
     'expected':{'action':'offer_passage','target':'unique','requires_confirmation':True,'preserve_saved':True}},
    {'id':'ambiguous_changed','saved_revision':'old','revision':'new','exact_source_exists':False,
     'passage_candidates':['first','second'],'chapter_candidates':['harbor'],
     'expected':{'action':'offer_chapter','target':'harbor','requires_confirmation':True,'preserve_saved':True}},
    {'id':'chapter_only','saved_revision':'old','revision':'new','exact_source_exists':False,
     'passage_candidates':[],'chapter_candidates':['harbor'],
     'expected':{'action':'offer_chapter','target':'harbor','requires_confirmation':True,'preserve_saved':True}},
    {'id':'no_match','saved_revision':'old','revision':'new','exact_source_exists':False,
     'passage_candidates':[],'chapter_candidates':[],
     'expected':{'action':'unresolved','target':None,'requires_confirmation':False,'preserve_saved':True}},
    {'id':'ambiguous_chapter','saved_revision':'old','revision':'new','exact_source_exists':False,
     'passage_candidates':[],'chapter_candidates':['harbor-one','harbor-two'],
     'expected':{'action':'unresolved','target':None,'requires_confirmation':False,'preserve_saved':True}},
]


def write_zip(path, files):
    with zipfile.ZipFile(path,'w') as z:
        for name, data in files.items():
            info = zipfile.ZipInfo(name, (2020,1,1,0,0,0))
            info.compress_type = zipfile.ZIP_STORED if name == 'mimetype' else zipfile.ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            z.writestr(info, data)


def generate(destination):
    destination = Path(destination)
    destination.mkdir(parents=True,exist_ok=True)
    for name, case in CASES.items():
        publication_title = 'Validation editions' if name.startswith('edition_') else f'Validation {name}'
        docs = case['documents']
        assets = case.get('assets',{})
        manifest = '<item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>'
        manifest += ''.join(f'<item id="s{i}" href="{p}" media-type="application/xhtml+xml"/>' for i,p in enumerate(docs))
        for i,p in enumerate(assets):
            mime = 'text/css' if p.endswith('.css') else 'image/svg+xml'
            manifest += f'<item id="asset{i}" href="{p}" media-type="{mime}"/>'
        spine = ''.join(f'<itemref idref="s{i}"/>' for i in range(len(docs)))
        nav = '<nav epub:type="toc"><ol>'+''.join(f'<li><a href="{escape(href)}">{escape(title)}</a></li>' for title,href in case['nav'])+'</ol></nav>'
        files = {'mimetype':'application/epub+zip',
                 'META-INF/container.xml':'<container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0"><rootfiles><rootfile full-path="package.opf" media-type="application/oebps-package+xml"/></rootfiles></container>',
                 'package.opf':f'<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="uid"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="uid">synthetic-shared-publisher-id</dc:identifier><dc:title>{publication_title}</dc:title><dc:language>en</dc:language><meta property="dcterms:modified">2020-01-01T00:00:00Z</meta></metadata><manifest>{manifest}</manifest><spine>{spine}</spine></package>',
                 'nav.xhtml':document(nav)}
        files.update({k:v for k,v in docs.items() if v is not None})
        files.update(assets)
        write_zip(destination / f'{name}.epub',files)
        (destination / f'{name}.expected.json').write_text(json.dumps(case['expected'],ensure_ascii=False,indent=2)+'\n')
    (destination/'not_a_zip.epub').write_bytes(b'Synthetic invalid container.\n')
    (destination/'restore.expected.json').write_text(json.dumps(RESTORE,indent=2)+'\n')
    return destination
