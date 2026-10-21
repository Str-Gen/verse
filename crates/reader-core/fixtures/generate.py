#!/usr/bin/env python3
"""Generate redistributable EPUB fixtures using only Python's standard library."""
from pathlib import Path
import binascii
import struct
import zipfile
import zlib

HERE = Path(__file__).resolve().parent
IDENTIFIER = "urn:synthetic:shared-publisher-id"


def png(width, height, mirror=False, photograph=False):
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", binascii.crc32(kind + data) & 0xffffffff)
    rows = bytearray()
    for y in range(height):
        rows.append(0)
        for x in range(width):
            xx = width - 1 - x if mirror else x
            if photograph:
                rows.extend(((xx * 7 + y * 3) % 256, (xx + y * 11) % 256, (xx * y) % 256, 255))
            else:
                ink = (xx < width // 3 and height // 3 <= y < 2 * height // 3) or (xx == y)
                rows.extend((30, 30, 30, 255) if ink else (255, 255, 255, 255))
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(bytes(rows))) + chunk(b"IEND", b"")


def archive(path, entries):
    with zipfile.ZipFile(path, "w") as output:
        for name, body in entries.items():
            info = zipfile.ZipInfo(name, (2020, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_STORED if name == "mimetype" else zipfile.ZIP_DEFLATED
            output.writestr(info, body)


def xhtml(title, body):
    return f'''<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><head><title>{title}</title><link rel="stylesheet" type="text/css" href="style.css"/></head><body>{body}</body></html>'''


def book(version="3.0", revised=False, unrelated=False):
    title = "Unrelated synthetic book" if unrelated else "Source-aware synthetic book"
    one = '''<h1 id="one">Chapter One</h1><p>Café naï<em>ve</em>té co<strong>op</strong>erate élan 👩‍🔬.</p>
<p>A <span class="gone">DISPLAY_SECRET</span>visible sentence.</p>
<p style="visibility:hidden">VISIBILITY_SECRET <span style="visibility:visible">Restored child.</span></p>
<p hidden="hidden">HTML_SECRET</p><p class="gone"><span style="display:block">DESCENDANT_SECRET</span></p>
<p class="cascade" style="display:block">Cascade visible.</p>
<p>Before <a epub:type="noteref" href="notes.xhtml#n1">1</a> after <a epub:type="noteref" href="notes.xhtml#missing">2</a>.</p>
<img src="ornament.png" alt=""/><h1 id="two">Chapter Two</h1><p>This chapter starts here.</p>
<p>state-of-the-art Donaudampfschifffahrt.</p>'''
    if revised:
        one = one.replace("visible sentence.", "visible revised sentence.")
    if unrelated:
        one = '<h1 id="one">Other story</h1><p>Unrelated planets orbit distant stars.</p><h1 id="two">Other continuation</h1><p>Different content entirely.</p>'
    two = '''<p>This chapter continues in a second spine file.</p><img src="ornament.png" alt=""/>
<img src="ornament-mirror.png" alt=""/><h1 id="three">Chapter Three</h1><p>Final chapter.</p>
<img src="photo.png" alt="A synthetic landscape"/><img src="uncertain.png" alt=""/>
<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><title>Vector diagram</title><circle cx="10" cy="10" r="8"/></svg>
<p>After unsupported vector content.</p>'''
    notes = '<aside id="n1" epub:type="footnote"><p>The manual note text.</p><a href="a.xhtml#one">Return</a></aside>'
    manifest = '''<item id="a" href="a.xhtml" media-type="application/xhtml+xml"/><item id="b" href="b.xhtml" media-type="application/xhtml+xml"/><item id="notes" href="notes.xhtml" media-type="application/xhtml+xml"/><item id="css" href="style.css" media-type="text/css"/>'''
    for name in ("ornament", "ornament-mirror", "photo", "uncertain"):
        manifest += f'<item id="{name}" href="{name}.png" media-type="image/png"/>'
    manifest += '<item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>'
    if version == "3.0":
        manifest += '<item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>'
    modified = '<meta property="dcterms:modified">2020-01-01T00:00:00Z</meta>' if version == "3.0" else ""
    opf = f'''<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="{version}" unique-identifier="uid"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="uid">{IDENTIFIER}</dc:identifier><dc:title>{title}</dc:title><dc:language>en</dc:language>{modified}</metadata><manifest>{manifest}</manifest><spine toc="ncx"><itemref idref="a"/><itemref idref="b"/><itemref idref="notes" linear="no"/></spine></package>'''
    destinations = [("One", "a.xhtml#one"), ("Two", "a.xhtml#two"), ("Three", "b.xhtml#three")]
    nav = '<nav epub:type="toc"><h1>Contents</h1><ol>' + ''.join(f'<li><a href="{href}">Chapter {label}</a></li>' for label, href in destinations) + '</ol></nav>'
    ncx = '<?xml version="1.0"?><ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1"><head><meta name="dtb:uid" content="' + IDENTIFIER + '"/></head><docTitle><text>' + title + '</text></docTitle><navMap>'
    ncx += ''.join(f'<navPoint id="c{i}" playOrder="{i}"><navLabel><text>Chapter {label}</text></navLabel><content src="{href}"/></navPoint>' for i, (label, href) in enumerate(destinations, 1)) + '</navMap></ncx>'
    return {
        "mimetype": "application/epub+zip",
        "META-INF/container.xml": '<?xml version="1.0"?><container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0"><rootfiles><rootfile full-path="EPUB/package.opf" media-type="application/oebps-package+xml"/></rootfiles></container>',
        "EPUB/package.opf": opf, "EPUB/a.xhtml": xhtml(title, one), "EPUB/b.xhtml": xhtml(title, two),
        "EPUB/notes.xhtml": xhtml("Notes", notes), "EPUB/nav.xhtml": xhtml("Contents", nav), "EPUB/toc.ncx": ncx,
        "EPUB/style.css": '.gone { display: none; } .cascade { display: none; }',
        "EPUB/ornament.png": png(48, 16), "EPUB/ornament-mirror.png": png(48, 16, mirror=True),
        "EPUB/photo.png": png(80, 64, photograph=True), "EPUB/uncertain.png": png(47, 17),
    }


def main():
    archive(HERE / "complex.epub", book())
    archive(HERE / "legacy.epub", book("2.0"))
    archive(HERE / "revised.epub", book(revised=True))
    archive(HERE / "unrelated.epub", book(unrelated=True))
    missing = book()
    del missing["EPUB/b.xhtml"]
    archive(HERE / "missing-resource.epub", missing)
    malformed = book()
    malformed["EPUB/a.xhtml"] = '<html><body><p>Unclosed'
    archive(HERE / "malformed-xhtml.epub", malformed)
    traversal = book()
    traversal["../escape.txt"] = "Traversal must be rejected."
    archive(HERE / "path-traversal.epub", traversal)
    (HERE / "not-zip.epub").write_bytes(b"Not an EPUB archive.\n")
    print("Generated eight synthetic EPUB fixtures.")


if __name__ == "__main__":
    main()
