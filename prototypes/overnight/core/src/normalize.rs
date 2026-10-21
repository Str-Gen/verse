use crate::{
    archive::{Archive, node_path, resolve, xml},
    semantics::style,
    *,
};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

struct Item {
    path: String,
    media: String,
    properties: String,
}
struct Navigation {
    destination: String,
    title: String,
}

/// Imports a bounded EPUB using an application-assigned identity. Publisher identifiers
/// remain metadata. Call `LocalStore::identity_for` to assign persistent local identities.
pub fn import_epub(path: &Path, book_id: &str, options: &ImportOptions) -> Result<Book> {
    let source_path = path;
    let _span = tracing::info_span!("epub_import", epub = %path.display()).entered();
    tracing::info!(code = "import_started", "Opening EPUB");
    let archive = Archive::open(path, &options.limits)?;
    let container = xml(
        archive.text("META-INF/container.xml")?,
        "META-INF/container.xml",
        &options.limits,
    )?;
    let package = container
        .descendants()
        .find(|n| n.has_tag_name("rootfile"))
        .and_then(|n| n.attribute("full-path"))
        .ok_or_else(|| Error::Invalid("missing package rootfile".into()))?;
    let (package, _) = resolve("", package)?;
    let opf = xml(archive.text(&package)?, &package, &options.limits)?;
    if opf.descendants().any(|n| {
        n.attribute("property") == Some("rendition:layout")
            && n.text().is_some_and(|text| text.trim() == "pre-paginated")
    }) {
        return Err(Error::Invalid(
            "fixed-layout publications are unsupported".into(),
        ));
    }
    let title = opf
        .descendants()
        .find(|n| n.has_tag_name("title"))
        .and_then(|n| n.text())
        .unwrap_or("Untitled")
        .to_owned();
    let publisher_identifiers = opf
        .descendants()
        .filter(|n| n.has_tag_name("identifier"))
        .filter_map(|n| n.text().map(str::to_owned))
        .collect();
    let mut manifest = BTreeMap::new();
    for n in opf.descendants().filter(|n| n.has_tag_name("item")) {
        let id = n
            .attribute("id")
            .ok_or_else(|| Error::Invalid("manifest item without id".into()))?;
        let href = n
            .attribute("href")
            .ok_or_else(|| Error::Invalid("manifest item without href".into()))?;
        let (path, _) = resolve(&package, href)?;
        if manifest
            .insert(
                id.to_owned(),
                Item {
                    path,
                    media: n.attribute("media-type").unwrap_or("").into(),
                    properties: n.attribute("properties").unwrap_or("").into(),
                },
            )
            .is_some()
        {
            return Err(Error::Invalid("duplicate manifest id".into()));
        }
    }
    let mut spine = Vec::new();
    for n in opf.descendants().filter(|n| n.has_tag_name("itemref")) {
        if n.attribute("properties").is_some_and(|p| {
            p.split_whitespace()
                .any(|p| p == "rendition:layout-pre-paginated")
        }) {
            return Err(Error::Invalid(
                "fixed-layout spine items are unsupported".into(),
            ));
        }
        let item = n
            .attribute("idref")
            .and_then(|id| manifest.get(id))
            .ok_or_else(|| Error::Invalid("unresolved spine item".into()))?;
        if item.media != "application/xhtml+xml" {
            return Err(Error::Invalid(
                "non-XHTML spine resource unsupported".into(),
            ));
        }
        spine.push((&item.path, n.attribute("linear") != Some("no")));
    }
    if spine.is_empty() {
        return Err(Error::Invalid("empty spine".into()));
    }
    let nav = navigation(&archive, &manifest, &options.limits)?;
    let mut book = Book {
        schema_version: 1,
        book_id: book_id.into(),
        revision: format!("{:x}", Sha256::digest(&archive.bytes)),
        title,
        publisher_identifiers,
        documents: Vec::new(),
        pieces: Vec::new(),
        chapters: Vec::new(),
        notes: Vec::new(),
        resources: Vec::new(),
        image_evidence: Vec::new(),
        diagnostics: Vec::new(),
    };
    let mut destinations = BTreeMap::new();
    let mut non_chapters = BTreeSet::new();
    let mut occurrences = Vec::new();
    let mut next_block = 0usize;
    // Parse all manifest XHTML, including non-spine note targets. Main text follows only linear spine order.
    let ordered: Vec<_> = spine
        .iter()
        .map(|(p, linear)| ((*p).clone(), *linear))
        .chain(
            manifest
                .values()
                .filter(|i| {
                    i.media == "application/xhtml+xml" && !spine.iter().any(|(p, _)| *p == &i.path)
                })
                .map(|i| (i.path.clone(), false)),
        )
        .collect();
    let mut visited = BTreeSet::new();
    let mut total_scalars = 0usize;
    for (path, linear) in ordered {
        if !visited.insert(path.clone()) {
            return Err(Error::Invalid("duplicate spine document".into()));
        }
        let doc = xml(archive.text(&path)?, &path, &options.limits)?;
        total_scalars = total_scalars
            .checked_add(
                doc.descendants()
                    .filter(|n| n.is_text())
                    .map(|n| n.text().unwrap_or("").chars().count())
                    .sum::<usize>(),
            )
            .ok_or(Error::Limit("publication text scalars"))?;
        if total_scalars > options.limits.text_scalars {
            return Err(Error::Limit("publication text scalars"));
        }
        let mut ids = BTreeSet::new();
        for n in doc.descendants().filter(|n| n.is_element()) {
            if let Some(id) = n.attribute("id") {
                if id.is_empty() || !ids.insert(id) {
                    let code = if id.is_empty() {
                        "empty_xhtml_id"
                    } else {
                        "duplicate_xhtml_id"
                    };
                    tracing::warn!(code, epub = %source_path.display(), document = %path, id, "Invalid XHTML ID; content retained without repair, fragment navigation may be ambiguous");
                    book.diagnostics.push(format!(
                        "{code}: {path} id={id:?}; content retained without repair"
                    ));
                }
                if frontmatter(n) {
                    non_chapters.insert(format!("{path}#{id}"));
                }
            }
        }
        let mut builder = Builder {
            path: &path,
            blocks: Vec::new(),
            current: None,
            next_block: &mut next_block,
            anchors: BTreeMap::new(),
            positions: BTreeMap::new(),
            occurrences: &mut occurrences,
            note_mode: false,
        };
        let body = doc
            .descendants()
            .find(|n| n.has_tag_name("body"))
            .ok_or_else(|| Error::Invalid("XHTML body missing".into()))?;
        if frontmatter(body) {
            non_chapters.insert(path.clone());
        }
        if linear {
            builder.walk(body)?;
            builder.flush();
        }
        let blocks = std::mem::take(&mut builder.blocks);
        let mut pieces = pieces_for(&blocks, options);
        for (id, (range, empty)) in &builder.anchors {
            if let Some(piece) = pieces.iter().find(|p| {
                p.sources.iter().any(|source| {
                    builder
                        .positions
                        .get(&source.node)
                        .is_some_and(|pos| range.contains(pos) || (*empty && *pos >= range.end))
                })
            }) {
                destinations.insert(format!("{path}#{id}"), piece.id.clone());
            }
        }
        if let Some(first) = pieces.first() {
            destinations.insert(path.clone(), first.id.clone());
        }
        if linear {
            book.pieces.append(&mut pieces);
            book.documents.push(Document {
                id: path.clone(),
                blocks,
            });
        }
        for note in body
            .descendants()
            .filter(|n| is_note(*n) && !n.ancestors().skip(1).any(is_note))
        {
            let Some(id) = note.attribute("id") else {
                book.diagnostics.push("semantic note lacks an ID".into());
                continue;
            };
            builder.note_mode = true;
            builder.walk(note)?;
            builder.flush();
            let blocks = std::mem::take(&mut builder.blocks);
            let pieces = pieces_for(&blocks, options);
            book.notes.push(Note {
                id: format!("{path}#{id}"),
                blocks,
                pieces,
            });
        }
    }
    let (evidence, resources) = ornaments::inspect(&archive, &options.limits, &occurrences)?;
    book.image_evidence = evidence;
    book.resources = resources;
    let mut image_kinds = BTreeMap::new();
    for block in book
        .documents
        .iter_mut()
        .flat_map(|d| &mut d.blocks)
        .chain(book.notes.iter_mut().flat_map(|n| &mut n.blocks))
    {
        if block.kind == "image" {
            let classification = block.resource.as_ref().map_or("unsupported", |resource| {
                ornaments::classification(&book.image_evidence, resource)
            });
            block.kind = match classification {
                "ornament" => "ornament",
                "substantive" => "image",
                _ => "unsupported",
            }
            .into();
            image_kinds.insert(block.id.clone(), block.kind.clone());
        }
    }
    let note_ids: BTreeSet<_> = book.notes.iter().map(|n| n.id.clone()).collect();
    for piece in book
        .pieces
        .iter_mut()
        .chain(book.notes.iter_mut().flat_map(|n| &mut n.pieces))
    {
        if let Some(kind) = image_kinds.get(&piece.block_id) {
            piece.kind = kind.clone();
        }
        if piece.kind == "note"
            && !piece
                .note_id
                .as_ref()
                .is_some_and(|id| note_ids.contains(id))
        {
            piece.kind = "unsupported".into();
            book.diagnostics
                .push("unresolved note reference; playback must pause".into());
        }
    }
    let mut seen_starts = BTreeSet::new();
    for entry in nav {
        if non_chapters.contains(&entry.destination) || frontmatter_title(&entry.title) {
            continue;
        }
        if let Some(start) = destinations.get(&entry.destination) {
            if seen_starts.insert(start.clone()) {
                book.chapters.push(Chapter {
                    id: entry.destination,
                    title: entry.title,
                    start_piece: start.clone(),
                    end_piece_exclusive: None,
                });
            } else {
                book.diagnostics
                    .push("duplicate navigation destination omitted".into());
            }
        } else {
            book.diagnostics
                .push("navigation target missing, hidden, or outside main text".into());
        }
    }
    let ranks: BTreeMap<_, _> = book
        .pieces
        .iter()
        .enumerate()
        .map(|(i, p)| (p.id.clone(), i))
        .collect();
    book.chapters
        .sort_by_key(|c| ranks.get(&c.start_piece).copied());
    for i in 0..book.chapters.len().saturating_sub(1) {
        book.chapters[i].end_piece_exclusive = Some(book.chapters[i + 1].start_piece.clone());
    }
    if book.chapters.is_empty() {
        book.diagnostics.push(
            "no resolved logical chapter navigation; no synthetic per-file boundaries invented"
                .into(),
        );
    }
    tracing::info!(
        code = "import_completed",
        documents = book.documents.len(),
        pieces = book.pieces.len(),
        diagnostics = book.diagnostics.len(),
        "EPUB imported"
    );
    Ok(book)
}

fn navigation(
    archive: &Archive,
    manifest: &BTreeMap<String, Item>,
    limits: &Limits,
) -> Result<Vec<Navigation>> {
    let mut entries = Vec::new();
    if let Some(item) = manifest
        .values()
        .find(|i| i.properties.split_whitespace().any(|p| p == "nav"))
    {
        let doc = xml(archive.text(&item.path)?, &item.path, limits)?;
        for nav in doc
            .descendants()
            .filter(|n| n.has_tag_name("nav") && semantic(*n, "toc"))
        {
            for a in nav.descendants().filter(|n| n.has_tag_name("a")) {
                if let Some(href) = a.attribute("href") {
                    let (path, fragment) = resolve(&item.path, href)?;
                    entries.push(Navigation {
                        destination: destination(path, fragment),
                        title: all_text(a),
                    });
                }
            }
        }
    } else if let Some(item) = manifest
        .values()
        .find(|i| i.media == "application/x-dtbncx+xml")
    {
        let doc = xml(archive.text(&item.path)?, &item.path, limits)?;
        for nav in doc.descendants().filter(|n| n.has_tag_name("navPoint")) {
            let content = nav
                .children()
                .find(|n| n.has_tag_name("content"))
                .and_then(|n| n.attribute("src"));
            let label = nav.children().find(|n| n.has_tag_name("navLabel"));
            if let (Some(href), Some(label)) = (content, label) {
                let (path, fragment) = resolve(&item.path, href)?;
                entries.push(Navigation {
                    destination: destination(path, fragment),
                    title: all_text(label),
                });
            }
        }
    }
    Ok(entries)
}
fn destination(path: String, fragment: Option<String>) -> String {
    fragment.map_or_else(|| path.clone(), |f| format!("{path}#{f}"))
}
fn all_text(n: roxmltree::Node<'_, '_>) -> String {
    n.descendants()
        .filter(|n| n.is_text())
        .filter_map(|n| n.text())
        .collect::<String>()
        .trim()
        .to_owned()
}
fn semantic(n: roxmltree::Node<'_, '_>, value: &str) -> bool {
    n.attribute(("http://www.idpf.org/2007/ops", "type"))
        .is_some_and(|s| s.split_whitespace().any(|s| s == value))
}
fn is_note(n: roxmltree::Node<'_, '_>) -> bool {
    n.is_element()
        && (semantic(n, "footnote")
            || semantic(n, "endnote")
            || matches!(n.attribute("role"), Some("doc-footnote" | "doc-endnote")))
}
fn frontmatter(n: roxmltree::Node<'_, '_>) -> bool {
    n.ancestors().any(|a| {
        [
            "frontmatter",
            "cover",
            "titlepage",
            "toc",
            "copyright-page",
            "dedication",
            "epigraph",
            "foreword",
            "preface",
            "acknowledgments",
            "part",
        ]
        .iter()
        .any(|kind| semantic(a, kind))
    })
}
fn frontmatter_title(title: &str) -> bool {
    matches!(
        title.trim().to_lowercase().as_str(),
        "cover"
            | "title page"
            | "titlepage"
            | "contents"
            | "table of contents"
            | "copyright"
            | "dedication"
            | "epigraph"
            | "foreword"
            | "preface"
            | "acknowledgments"
            | "acknowledgements"
    )
}
struct Builder<'a> {
    path: &'a str,
    blocks: Vec<Block>,
    current: Option<Block>,
    next_block: &'a mut usize,
    anchors: BTreeMap<String, (std::ops::Range<usize>, bool)>,
    positions: BTreeMap<String, usize>,
    occurrences: &'a mut Vec<(String, bool)>,
    note_mode: bool,
}
impl Builder<'_> {
    fn block(&mut self, kind: &str) -> Block {
        let id = format!("b{}", self.next_block);
        *self.next_block += 1;
        Block {
            id,
            kind: kind.into(),
            runs: Vec::new(),
            level: None,
            alt: None,
            resource: None,
            note_id: None,
        }
    }
    fn flush(&mut self) {
        if let Some(b) = self.current.take()
            && (!b.runs.is_empty() || b.kind != "paragraph")
        {
            self.blocks.push(b);
        }
    }
    fn walk(&mut self, n: roxmltree::Node<'_, '_>) -> Result<()> {
        if n.is_text() {
            let Some(parent) = n.parent_element() else {
                return Ok(());
            };
            let style = style(parent);
            if style.display_none {
                return Ok(());
            }
            let text = n.text().unwrap_or("");
            if self.current.is_none() && text.trim().is_empty() {
                return Ok(());
            }
            if self.current.is_none() {
                self.current = Some(self.block("paragraph"));
            }
            let note_id = n
                .ancestors()
                .find(|a| semantic(*a, "noteref") || a.attribute("role") == Some("doc-noteref"))
                .map(|a| {
                    a.attribute("href")
                        .and_then(|href| resolve(self.path, href).ok())
                        .map(|(p, f)| destination(p, f))
                        .unwrap_or_default()
                });
            let node = node_path(n);
            self.positions.insert(node.clone(), n.range().start);
            if let Some(block) = &mut self.current {
                block.runs.push(Run {
                    note_id,
                    text: text.into(),
                    emphasis: style.emphasis,
                    strong: style.strong,
                    sources: vec![SourceSpan {
                        document: self.path.into(),
                        node,
                        offset: 0,
                        length: text.chars().count(),
                    }],
                });
            }
            return Ok(());
        }
        if !n.is_element() {
            return Ok(());
        }
        let tag = n.tag_name().name();
        if matches!(tag, "head" | "script" | "style" | "template") {
            return Ok(());
        }
        let style = style(n);
        if style.display_none {
            return Ok(());
        }
        if is_note(n) && !self.note_mode {
            return Ok(());
        }
        if let Some(id) = n.attribute("id") {
            let empty = n.children().all(|c| {
                c.is_comment() || (c.is_text() && c.text().unwrap_or("").trim().is_empty())
            });
            self.anchors.insert(id.into(), (n.range(), empty));
        }
        if matches!(
            tag,
            "img"
                | "svg"
                | "math"
                | "table"
                | "audio"
                | "video"
                | "object"
                | "iframe"
                | "canvas"
                | "form"
                | "input"
                | "select"
                | "textarea"
                | "button"
        ) {
            self.flush();
            let mut block = self.block(if tag == "img" { "image" } else { "unsupported" });
            block.alt = Some(
                n.attribute("alt")
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("Unsupported {tag}")),
            );
            if tag == "img"
                && let Some(src) = n.attribute("src")
                && let Ok((resource, _)) = resolve(self.path, src)
            {
                let decorative = matches!(n.attribute("role"), Some("presentation" | "none"))
                    && n.attribute("alt") == Some("");
                self.occurrences.push((resource.clone(), decorative));
                block.resource = Some(resource);
            }
            let node = node_path(n);
            self.positions.insert(node.clone(), n.range().start);
            block.runs.push(Run {
                note_id: None,
                text: String::new(),
                emphasis: false,
                strong: false,
                sources: vec![SourceSpan {
                    document: self.path.into(),
                    node,
                    offset: 0,
                    length: 0,
                }],
            });
            self.blocks.push(block);
            return Ok(());
        }
        if tag == "br" {
            self.flush();
            return Ok(());
        }
        let block_kind = match tag {
            "p" | "blockquote" | "pre" => Some("paragraph"),
            "li" => Some("list_item"),
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => Some("heading"),
            _ => None,
        };
        let container = matches!(
            tag,
            "div" | "section" | "article" | "aside" | "ul" | "ol" | "body"
        );
        if block_kind.is_some() || container {
            self.flush();
        }
        if let Some(kind) = block_kind {
            let mut b = self.block(kind);
            if kind == "heading" {
                b.level = tag[1..].parse().ok();
            }
            self.current = Some(b);
        }
        for child in n.children() {
            self.walk(child)?;
        }
        if block_kind.is_some() || container {
            self.flush();
        }
        Ok(())
    }
}

fn pieces_for(blocks: &[Block], options: &ImportOptions) -> Vec<Piece> {
    let mut result = Vec::new();
    for block in blocks {
        if !block.runs.iter().any(|r| r.note_id.is_some()) {
            result.extend(text_pieces(std::slice::from_ref(block), options));
            continue;
        }
        let mut groups: Vec<Vec<Run>> = Vec::new();
        for run in &block.runs {
            if let Some(group) = groups
                .last_mut()
                .filter(|g| g.first().is_some_and(|r| r.note_id == run.note_id))
            {
                group.push(run.clone());
            } else {
                groups.push(vec![run.clone()]);
            }
        }
        for (i, runs) in groups.into_iter().enumerate() {
            let mut segment = block.clone();
            segment.id = format!("{}:g{i}", block.id);
            segment.runs = runs;
            if let Some(note_id) = segment.runs.first().and_then(|r| r.note_id.clone()) {
                result.push(Piece {
                    id: format!("{}:note", segment.id),
                    text: segment.runs.iter().map(|r| r.text.as_str()).collect(),
                    word_id: String::new(),
                    sources: segment
                        .runs
                        .iter()
                        .flat_map(|r| r.sources.clone())
                        .collect(),
                    block_id: block.id.clone(),
                    paragraph_end: false,
                    kind: "note".into(),
                    note_id: Some(note_id),
                });
            } else {
                let mut pieces = text_pieces(&[segment], options);
                for p in &mut pieces {
                    p.block_id = block.id.clone();
                    p.paragraph_end = false;
                }
                result.extend(pieces);
            }
        }
        if let Some(last) = result.last_mut().filter(|p| p.block_id == block.id) {
            last.paragraph_end = true;
        }
    }
    result
}

fn text_pieces(blocks: &[Block], options: &ImportOptions) -> Vec<Piece> {
    let mut pieces = Vec::new();
    for block in blocks {
        if matches!(
            block.kind.as_str(),
            "image" | "ornament" | "unsupported" | "note"
        ) {
            pieces.push(Piece {
                id: format!("{}:event", block.id),
                text: block.alt.clone().unwrap_or_default(),
                word_id: String::new(),
                sources: block.runs.iter().flat_map(|r| r.sources.clone()).collect(),
                block_id: block.id.clone(),
                paragraph_end: false,
                kind: block.kind.clone(),
                note_id: block.note_id.clone(),
            });
            continue;
        }
        let text: String = block.runs.iter().map(|r| r.text.as_str()).collect();
        let chars: Vec<char> = text.chars().collect();
        let byte_starts: Vec<_> = text
            .char_indices()
            .map(|(byte, _)| byte)
            .chain(std::iter::once(text.len()))
            .collect();
        let mut source_ranges = Vec::new();
        let mut scalar_offset = 0;
        for run in &block.runs {
            for source in &run.sources {
                source_ranges.push((scalar_offset..scalar_offset + source.length, source));
                scalar_offset += source.length;
            }
        }
        // Display passages are whitespace-delimited. Unicode word boundaries can
        // split A😀B and attach the next symbol across a space to the prior word.
        let mut ranges = Vec::new();
        let mut start = None;
        for (byte, character) in text.char_indices() {
            if character.is_whitespace() {
                if let Some(begin) = start.take() {
                    ranges.push((begin, byte));
                }
            } else if start.is_none() {
                start = Some(byte);
            }
        }
        if let Some(begin) = start {
            ranges.push((begin, text.len()));
        }
        for i in 0..ranges.len() {
            let (lexical_start, lexical_end) = ranges[i];
            let start = if i == 0 { 0 } else { lexical_start };
            let end = if i + 1 < ranges.len() {
                ranges[i + 1].0
            } else {
                text.len()
            };
            let slice = &text[start..end];
            let trimmed = slice.trim();
            if trimmed.is_empty() {
                continue;
            }
            let byte_start = start + slice.len() - slice.trim_start().len();
            let scalar_start = byte_starts.partition_point(|b| *b < byte_start);
            let scalar_end = scalar_start + trimmed.chars().count();
            let lexical = &text[lexical_start..lexical_end];
            let word_id = format!("{}:w{i}", block.id);
            let mut cuts = vec![scalar_start];
            if options.split_compounds.contains(lexical) {
                cuts.extend(
                    (scalar_start..scalar_end)
                        .filter(|j| matches!(chars[*j], '-' | '\u{2010}' | '\u{2011}'))
                        .map(|j| j + 1),
                );
            }
            cuts.push(scalar_end);
            cuts.dedup();
            for (part, range) in cuts.windows(2).enumerate() {
                let text: String = chars[range[0]..range[1]].iter().collect();
                let first = source_ranges.partition_point(|(r, _)| r.end <= range[0]);
                let sources = source_ranges[first..]
                    .iter()
                    .take_while(|(r, _)| r.start < range[1])
                    .map(|(r, s)| {
                        let mut span = (*s).clone();
                        let start = r.start.max(range[0]);
                        let end = r.end.min(range[1]);
                        span.offset += start - r.start;
                        span.length = end - start;
                        span
                    })
                    .collect();
                pieces.push(Piece {
                    id: format!("{word_id}:{part}"),
                    text,
                    word_id: word_id.clone(),
                    sources,
                    block_id: block.id.clone(),
                    paragraph_end: false,
                    kind: "word".into(),
                    note_id: None,
                });
            }
        }
        if let Some(last) = pieces.last_mut().filter(|p| p.block_id == block.id) {
            last.paragraph_end = true;
        }
    }
    pieces
}
/// Independently reopens XML and checks every exported visible run and word against
/// decoded source addresses. Returns the number of checked pieces, without book text.
pub fn verify_sources(path: &Path, book: &Book, limits: &Limits) -> Result<usize> {
    let archive = Archive::open(path, limits)?;
    if format!("{:x}", Sha256::digest(&archive.bytes)) != book.revision {
        return Err(Error::Invalid(
            "source verification revision mismatch".into(),
        ));
    }
    let mut nodes = BTreeMap::new();
    for (path, bytes) in &archive.files {
        if !(path.ends_with(".xhtml") || path.ends_with(".html") || path.ends_with(".htm")) {
            continue;
        }
        let text =
            std::str::from_utf8(bytes).map_err(|_| Error::Invalid("source XML encoding".into()))?;
        let doc = xml(text, path, limits)?;
        for n in doc.descendants() {
            nodes.insert(
                (path.clone(), node_path(n)),
                n.text().unwrap_or("").chars().collect::<Vec<_>>(),
            );
        }
    }
    let extract = |spans: &[SourceSpan]| -> Result<String> {
        let mut text = String::new();
        for s in spans {
            let source = nodes
                .get(&(s.document.clone(), s.node.clone()))
                .ok_or_else(|| Error::Invalid("source node absent".into()))?;
            let chars = source;
            let end = s
                .offset
                .checked_add(s.length)
                .ok_or(Error::Limit("source offset"))?;
            let part = chars
                .get(s.offset..end)
                .ok_or_else(|| Error::Invalid("source range absent".into()))?;
            text.extend(part);
        }
        Ok(text)
    };
    for block in book
        .documents
        .iter()
        .flat_map(|d| &d.blocks)
        .chain(book.notes.iter().flat_map(|n| &n.blocks))
    {
        for run in &block.runs {
            if extract(&run.sources)? != run.text {
                return Err(Error::Invalid("run/source mismatch".into()));
            }
        }
    }
    let mut count = 0;
    for p in book
        .pieces
        .iter()
        .chain(book.notes.iter().flat_map(|n| &n.pieces))
    {
        let source = extract(&p.sources)?;
        if (p.kind == "word" || p.sources.iter().any(|s| s.length > 0)) && source != p.text {
            return Err(Error::Invalid("piece/source mismatch".into()));
        }
        count += 1;
    }
    Ok(count)
}

/// Candidate evaluation against the exact bounded archive, without publisher text output.
pub fn inspect_rbook(path: &Path, limits: &Limits) -> Result<usize> {
    let archive = Archive::open(path, limits)?;
    let epub = rbook::Epub::read(std::io::Cursor::new(archive.bytes))
        .map_err(|_| Error::Invalid("rbook could not parse bounded archive".into()))?;
    Ok(epub.spine().len())
}
