# Synthetic EPUB fixtures

Run `python3 fixtures/generate.py` from the core directory. The generator uses the Python standard library, writes deterministic ZIP timestamps, and contains all text and image generation source. Every asset is synthetic and redistributable. It reads no private books.

`complex.epub` is EPUB3; `legacy.epub` uses an EPUB2 package and NCX navigation. `revised.epub` changes one visible sentence. `unrelated.epub` changes the title and opening content while deliberately retaining the same publisher identifier as all other valid fixtures. Those identifiers must not merge local book identities.

## Visible text and correspondence

The first paragraph must read `Café naïveté cooperate élan 👩‍🔬.`. The final accent in `élan` is a combining U+0301 after `e`, rather than precomposed `é`. `naïveté` joins three decoded source text nodes, with `ve` emphasized; `cooperate` joins three nodes, with `op` strong. Each remains one lexical word. Source offsets count Unicode scalar values, not bytes or UTF-16 units.

The normal reading sequence contains these paragraphs, with headings, images, and note references in their source positions:

```
Chapter One
Café naïveté cooperate élan 👩‍🔬.
A DISPLAY_SECRETvisible sentence.
VISIBILITY_SECRET Restored child.
DESCENDANT_SECRET
Cascade visible.
Before 1 after 2.
Chapter Two
This chapter starts here.
state-of-the-art Donaudampfschifffahrt.
This chapter continues in a second spine file.
Chapter Three
Final chapter.
After unsupported vector content.
```

Only `HTML_SECRET` is excluded by the HTML `hidden` attribute. The other markers are hidden only by publisher CSS and remain readable under Verse's own typography policy. Inline `em` and `strong` semantics remain intact.

The source spelling of `state-of-the-art` and `Donaudampfschifffahrt` is preserved while hyphenated words automatically split into playback beats. Tests should distinguish lexical word identity from display-piece identity.

The manual note target is `EPUB/notes.xhtml#n1`, containing `The manual note text.` and a return link. It is in a non-linear spine resource and should not silently become ordinary main-text playback. The second noteref points to missing `notes.xhtml#missing` and requires an explicit unresolved state. `Vector diagram` is an inline SVG title; the SVG requires an unsupported-content marker or supported equivalent, never silent disappearance.

## Logical chapters

The EPUB3 nav and EPUB2 NCX both define exactly three chapter destinations:

| Title | Destination | Boundary expectation |
| --- | --- | --- |
| Chapter One | `a.xhtml#one` | Begins at first heading |
| Chapter Two | `a.xhtml#two` | Begins within first spine file and continues through opening paragraph of `b.xhtml` |
| Chapter Three | `b.xhtml#three` | Begins within second spine file |

Starting the second spine file is not a chapter boundary. Notes are not a fourth logical chapter. The changed edition only changes `A visible sentence.` to `A visible revised sentence.`, so unchanged passages provide evidence for proposed recovery. A recovery proposal still requires confirmation.

## Images

`ornament.png` occurs twice across the two main spine files. `ornament-mirror.png` is an exact horizontal pixel mirror, with different encoded bytes. Both are 48 × 16 grayscale synthetic ornaments. This gives repeated and mirrored appearance evidence, not a universal proof that every repeated image is decorative. `photo.png` is a synthetic colored 80 × 64 image with substantive alt text. `uncertain.png` is a unique 47 × 17 shape with empty alt text and must default to substantive unless stronger justified evidence is available. Empty alt text and small dimensions alone are insufficient.

## Malformed cases

`missing-resource.epub` omits the second spine resource. `malformed-xhtml.epub` contains an unclosed first XHTML document. `path-traversal.epub` contains a `../escape.txt` entry and must never write outside any destination directory. `not-zip.epub` is plain text. Exact error or warning behavior belongs to the normalizer, but successful output must not silently claim complete content when a required resource is missing or malformed. Resource-limit tests can generate bounded archives at test runtime without committing large compressed bombs.

`pixels.avif` is a synthetic 4 × 3 solid-color test image generated with `magick -size 4x3 xc:'#356ca5' pixels.avif`. It exercises actual AVIF decoding and PNG export.
