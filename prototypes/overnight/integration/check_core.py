#!/usr/bin/env python3
"""Run the independent oracle, retaining the unsupported-SVG failure separately."""
import copy
import json
from pathlib import Path
import struct
import subprocess
import sys
import zlib

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE/'../validation'))
from fixtures import CASES, generate
from validate import run_core, validate_export
from validation_adapter import adapt

def png(width, height, color):
    def chunk(kind, data):
        return struct.pack('>I',len(data)) + kind + data + struct.pack('>I',zlib.crc32(kind+data))
    pixels = b''.join(b'\0' + bytes(color)*width for _ in range(height))
    return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR',struct.pack('>IIBBBBB',width,height,8,2,0,0,0)) + chunk(b'IDAT',zlib.compress(pixels)) + chunk(b'IEND',b'')

def main():
    output = HERE/'evidence/independent'
    code = run_core([sys.executable, str(HERE/'validation_adapter.py')], output, 30)
    original = json.loads((output/'results.json').read_text())
    failures = [r for r in original['results'] if r['status'] != 'pass']
    assert [r['case'] for r in failures] == ['images_unsupported'], failures
    raw = json.loads((output/'exports/images_unsupported.json').read_text())
    assert sum(p['kind'] == 'unsupported' for p in raw['pieces']) == 5
    case = copy.deepcopy(CASES['images_unsupported'])
    # Same source structure and independent expected behavior, using supported
    # raster assets. The original SVG case and its failure remain unchanged.
    case['documents']['art.xhtml'] = case['documents']['art.xhtml'].replace('.svg', '.png')
    case['assets'] = {'ornament.png':png(80, 12, (30,30,30)),
                      'landscape.png':png(320, 180, (40,140,60))}
    CASES['images_raster'] = case
    fixture_dir = generate(output/'raster-fixtures')
    epub = fixture_dir/'images_raster.epub'
    export = output/'raster.json'
    subprocess.run([str(HERE/'../core/target/debug/reader-core'),'export','--epub',str(epub),'--output',str(export)],check=True)
    book = adapt(json.loads(export.read_text()), epub)
    result = validate_export('images_raster',book)
    (output/'raster-result.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps({'original_suite_exit':code,'known_failure':failures,'supported_raster':result},indent=2))

if __name__ == '__main__':
    main()
