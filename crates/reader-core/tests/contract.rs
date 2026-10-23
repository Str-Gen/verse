use reader_core::location::{LocalStore, Recovery, recover_location};
use reader_core::{Book, Error, ImportOptions, Limits, import_epub, verify_sources};
use std::{error::Error as StdError, path::PathBuf};

type TestResult = Result<(), Box<dyn StdError>>;
fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name)
}
fn load(name: &str) -> reader_core::Result<Book> {
    import_epub(&fixture(name), "synthetic-local", &ImportOptions::default())
}
fn block_text(book: &Book) -> Vec<String> {
    book.documents
        .iter()
        .flat_map(|d| &d.blocks)
        .filter(|b| matches!(b.kind.as_str(), "paragraph" | "heading"))
        .map(|b| b.runs.iter().map(|r| r.text.as_str()).collect::<String>())
        .filter(|s| !s.trim().is_empty())
        .collect()
}

#[test]
fn verse_preserves_unicode_inline_semantics_and_html_hidden() -> TestResult {
    let book = load("complex.epub")?;
    let visible = block_text(&book);
    assert!(
        visible
            .iter()
            .any(|s| s == "Café naïveté cooperate élan 👩‍🔬.")
    );
    assert!(
        visible
            .iter()
            .any(|s| s == "A DISPLAY_SECRETvisible sentence.")
    );
    assert!(
        visible
            .iter()
            .any(|s| s == "VISIBILITY_SECRET Restored child.")
    );
    assert!(visible.iter().any(|s| s == "Cascade visible."));
    let exported = serde_json::to_string(&book)?;
    assert!(!exported.contains("HTML_SECRET"));
    for publisher_hidden in ["DISPLAY_SECRET", "VISIBILITY_SECRET", "DESCENDANT_SECRET"] {
        assert!(exported.contains(publisher_hidden));
    }
    let naive = book
        .pieces
        .iter()
        .find(|p| p.text == "naïveté")
        .ok_or("whole naïveté missing")?;
    assert_eq!(naive.sources.len(), 3);
    assert_eq!(
        naive.sources.iter().map(|s| s.length).collect::<Vec<_>>(),
        [3, 2, 2]
    );
    assert_eq!(
        naive.sources[0].offset, 5,
        "Café plus space is five Unicode scalars"
    );
    let cooperate = book
        .pieces
        .iter()
        .find(|p| p.text == "cooperate")
        .ok_or("whole cooperate missing")?;
    assert_eq!(
        cooperate
            .sources
            .iter()
            .map(|s| s.length)
            .collect::<Vec<_>>(),
        [2, 2, 5]
    );
    assert!(
        book.documents
            .iter()
            .flat_map(|d| &d.blocks)
            .flat_map(|b| &b.runs)
            .any(|r| r.text == "ve" && r.emphasis)
    );
    assert!(
        book.documents
            .iter()
            .flat_map(|d| &d.blocks)
            .flat_map(|b| &b.runs)
            .any(|r| r.text == "op" && r.strong)
    );
    assert!(
        verify_sources(&fixture("complex.epub"), &book, &Limits::default())? >= book.pieces.len()
    );
    Ok(())
}

#[test]
fn logical_chapters_cross_spine_files_and_use_fragments() -> TestResult {
    let book = load("complex.epub")?;
    assert_eq!(
        book.chapters
            .iter()
            .map(|c| c.id.as_str())
            .collect::<Vec<_>>(),
        ["EPUB/a.xhtml#one", "EPUB/a.xhtml#two", "EPUB/b.xhtml#three"]
    );
    for (chapter, document) in
        book.chapters
            .iter()
            .zip(["EPUB/a.xhtml", "EPUB/a.xhtml", "EPUB/b.xhtml"])
    {
        let first = book
            .pieces
            .iter()
            .find(|p| p.id == chapter.start_piece)
            .ok_or("chapter start absent")?;
        assert_eq!(first.text, "Chapter");
        assert_eq!(first.sources[0].document, document);
    }
    let second = &book.chapters[1];
    let start = book
        .pieces
        .iter()
        .position(|p| p.id == second.start_piece)
        .ok_or("second start absent")?;
    let end = book
        .pieces
        .iter()
        .position(|p| Some(&p.id) == second.end_piece_exclusive.as_ref())
        .ok_or("second end absent")?;
    let range = &book.pieces[start..end];
    assert!(
        range
            .iter()
            .any(|p| p.sources.iter().any(|s| s.document == "EPUB/a.xhtml"))
    );
    assert!(
        range
            .iter()
            .any(|p| p.sources.iter().any(|s| s.document == "EPUB/b.xhtml"))
    );
    let text = range
        .iter()
        .map(|p| p.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(text.contains("This chapter continues in a second spine file."));
    assert!(book.chapters[2].end_piece_exclusive.is_none());
    Ok(())
}

#[test]
fn manual_notes_missing_targets_and_unsupported_content_remain_visible() -> TestResult {
    let book = load("complex.epub")?;
    assert_eq!(book.notes.len(), 1);
    assert_eq!(book.notes[0].id, "EPUB/notes.xhtml#n1");
    let note_text = book.notes[0]
        .blocks
        .iter()
        .flat_map(|b| &b.runs)
        .map(|r| r.text.as_str())
        .collect::<String>();
    assert!(note_text.contains("The manual note text."));
    assert!(!book.pieces.iter().any(|p| p.text == "manual"));
    assert!(
        book.pieces
            .iter()
            .any(|p| p.kind == "note" && p.note_id.as_deref() == Some("EPUB/notes.xhtml#n1"))
    );
    assert!(book.pieces.iter().any(
        |p| p.kind == "unsupported" && p.note_id.as_deref() == Some("EPUB/notes.xhtml#missing")
    ));
    assert!(
        book.diagnostics
            .iter()
            .any(|s| s.contains("unresolved note"))
    );
    assert!(
        book.documents
            .iter()
            .flat_map(|d| &d.blocks)
            .any(|b| b.kind == "unsupported" && b.note_id.is_none())
    );
    assert!(
        book.pieces
            .iter()
            .filter(|p| p.kind == "unsupported")
            .count()
            >= 2
    );
    assert_eq!(book.pieces.iter().filter(|p| p.kind == "image").count(), 5);
    let repeated = book
        .image_evidence
        .iter()
        .find(|e| e.resource == "EPUB/ornament.png")
        .ok_or("image evidence missing")?;
    assert_eq!(repeated.exact_occurrences, 2);
    assert_eq!(repeated.mirrored_or_identical_assets, 2);
    assert!(
        book.image_evidence
            .iter()
            .all(|e| e.classification == "substantive")
    );
    assert_eq!(book.resources.len(), 4);
    Ok(())
}

#[test]
fn epub2_and_epub3_agree_on_reader_content() -> TestResult {
    let modern = load("complex.epub")?;
    let legacy = load("legacy.epub")?;
    assert_eq!(block_text(&modern), block_text(&legacy));
    assert_eq!(
        modern
            .pieces
            .iter()
            .map(|p| (&p.text, &p.kind))
            .collect::<Vec<_>>(),
        legacy
            .pieces
            .iter()
            .map(|p| (&p.text, &p.kind))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        modern
            .chapters
            .iter()
            .map(|c| (&c.id, &c.title))
            .collect::<Vec<_>>(),
        legacy
            .chapters
            .iter()
            .map(|c| (&c.id, &c.title))
            .collect::<Vec<_>>()
    );
    assert_ne!(modern.revision, legacy.revision);
    for (name, book) in [("complex.epub", &modern), ("legacy.epub", &legacy)] {
        verify_sources(&fixture(name), book, &Limits::default())?;
    }
    Ok(())
}

#[test]
fn publisher_ids_never_merge_local_identities_and_store_reopens_exactly() -> TestResult {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("scratch")
        .join(format!("contract-{}", uuid::Uuid::new_v4()));
    let store = LocalStore::open(&root)?;
    let first_id = store.identity_for(fixture("complex.epub"))?;
    let other_id = store.identity_for(fixture("unrelated.epub"))?;
    let first = import_epub(
        &fixture("complex.epub"),
        &first_id,
        &ImportOptions::default(),
    )?;
    let other = import_epub(
        &fixture("unrelated.epub"),
        &other_id,
        &ImportOptions::default(),
    )?;
    assert_eq!(first.publisher_identifiers, other.publisher_identifiers);
    assert_ne!(first.book_id, other.book_id);
    assert_ne!(first.revision, other.revision);
    let piece = first
        .pieces
        .iter()
        .find(|p| p.text == "naïveté")
        .ok_or("missing target")?;
    store.save_location(&first, &piece.id)?;
    let reopened = LocalStore::open(&root)?;
    assert_eq!(first_id, reopened.identity_for(fixture("complex.epub"))?);
    let saved = reopened
        .load_location(&first_id)?
        .ok_or("missing saved location")?;
    assert_eq!(
        recover_location(&first, &saved),
        Recovery::Exact {
            piece_id: piece.id.clone()
        }
    );
    assert!(matches!(
        recover_location(&other, &saved),
        Recovery::Preserve { .. }
    ));
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn malformed_publications_and_resource_limits_fail_explicitly() -> TestResult {
    for name in [
        "not-zip.epub",
        "path-traversal.epub",
        "missing-resource.epub",
        "malformed-xhtml.epub",
    ] {
        assert!(
            load(name).is_err(),
            "malformed fixture unexpectedly accepted: {name}"
        );
    }
    let limits = [
        Limits {
            archive_bytes: 8,
            ..Limits::default()
        },
        Limits {
            entries: 2,
            ..Limits::default()
        },
        Limits {
            entry_bytes: 32,
            ..Limits::default()
        },
        Limits {
            expanded_bytes: 32,
            ..Limits::default()
        },
        Limits {
            xml_nodes: 2,
            ..Limits::default()
        },
        Limits {
            xml_depth: 2,
            ..Limits::default()
        },
        Limits {
            image_pixels: 10,
            ..Limits::default()
        },
        Limits {
            text_scalars: 10,
            ..Limits::default()
        },
    ];
    for (index, limits) in limits.into_iter().enumerate() {
        let result = import_epub(
            &fixture("complex.epub"),
            "bounded",
            &ImportOptions { limits },
        );
        assert!(
            matches!(
                result,
                Err(Error::Limit(_) | Error::Invalid(_) | Error::ImageTooLarge { .. })
            ),
            "resource limit {index} did not reject"
        );
    }
    Ok(())
}

#[test]
fn display_splitting_keeps_lexical_identity_and_changed_editions_only_propose() -> TestResult {
    let split = load("complex.epub")?;
    let first = split
        .pieces
        .iter()
        .position(|p| p.text == "state-")
        .ok_or("compound absent")?;
    let word_id = &split.pieces[first].word_id;
    let displayed: Vec<_> = split
        .pieces
        .iter()
        .filter(|p| &p.word_id == word_id)
        .collect();
    let mut original = split.clone();
    let mut whole = displayed[0].clone();
    whole.text = displayed.iter().map(|p| p.text.as_str()).collect();
    whole.sources.truncate(1);
    whole.sources[0].length = whole.text.chars().count();
    whole.paragraph_end = displayed.last().ok_or("no parts")?.paragraph_end;
    original
        .pieces
        .splice(first..first + displayed.len(), [whole.clone()]);
    let compound = &whole;
    let saved = reader_core::location::SavedLocation::capture(&original, &compound.id)?;
    let current_saved = reader_core::location::SavedLocation::capture(&split, &displayed[0].id)?;
    assert_eq!(
        displayed
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>(),
        ["state-", "of-", "the-", "art"]
    );
    assert_eq!(
        displayed
            .iter()
            .flat_map(|p| &p.sources)
            .map(|s| s.length)
            .sum::<usize>(),
        compound.text.chars().count()
    );
    assert_eq!(
        recover_location(&split, &saved),
        Recovery::Exact {
            piece_id: displayed[0].id.clone()
        }
    );
    let cached = reader_core::session::book_from_value(serde_json::to_value(&original)?)?;
    assert_eq!(
        serde_json::to_value(&cached)?,
        serde_json::to_value(&split)?
    );
    let resume = reader_core::session::restore(
        &cached,
        Some(&serde_json::json!({"revision": original.revision, "location": saved})),
    )?;
    assert_eq!(resume.kind, "exact");
    assert_eq!(resume.at, first);
    for part in &displayed {
        let location = reader_core::location::SavedLocation::capture(&split, &part.id)?;
        assert_eq!(
            recover_location(&cached, &location),
            Recovery::Exact {
                piece_id: part.id.clone()
            }
        );
    }
    verify_sources(&fixture("complex.epub"), &split, &Limits::default())?;
    let revised = import_epub(
        &fixture("revised.epub"),
        &original.book_id,
        &ImportOptions::default(),
    )?;
    assert_ne!(revised.revision, original.revision);
    assert!(matches!(
        recover_location(&revised, &current_saved),
        Recovery::ProposePassage { .. }
    ));
    assert_eq!(saved.revision, original.revision);
    assert_eq!(saved.text, compound.text);
    Ok(())
}

struct ModifiedFixture {
    root: PathBuf,
    path: PathBuf,
}
impl ModifiedFixture {
    fn new(replacements: &[(&str, &[u8])]) -> Result<Self, Box<dyn StdError>> {
        use std::io::{Read, Write};
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("scratch")
            .join(format!("modified-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root)?;
        let path = root.join("book.epub");
        let mut source = zip::ZipArchive::new(std::fs::File::open(fixture("complex.epub"))?)?;
        let mut output = zip::ZipWriter::new(std::fs::File::create(&path)?);
        let mut copied = std::collections::BTreeSet::new();
        for index in 0..source.len() {
            let mut entry = source.by_index(index)?;
            let name = entry.name().to_owned();
            let mut original = Vec::new();
            entry.read_to_end(&mut original)?;
            let data = replacements
                .iter()
                .find(|(p, _)| *p == name)
                .map_or(original.as_slice(), |(_, bytes)| *bytes);
            let method = if name == "mimetype" {
                zip::CompressionMethod::Stored
            } else {
                zip::CompressionMethod::Deflated
            };
            output.start_file(
                &name,
                zip::write::SimpleFileOptions::default().compression_method(method),
            )?;
            output.write_all(data)?;
            copied.insert(name);
        }
        for (name, data) in replacements
            .iter()
            .filter(|(name, _)| !copied.contains(*name))
        {
            output.start_file(
                *name,
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated),
            )?;
            output.write_all(data)?;
        }
        output.finish()?;
        Ok(Self { root, path })
    }
    fn import(&self) -> reader_core::Result<Book> {
        import_epub(&self.path, "modified-fixture", &ImportOptions::default())
    }
}
impl Drop for ModifiedFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn synthetic_xhtml(body: &str, head: &str) -> String {
    format!(
        r#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><head><title>Synthetic variant</title>{head}</head><body>{body}</body></html>"#
    )
}
fn synthetic_nav(links: &[(&str, &str)]) -> String {
    let list = links
        .iter()
        .map(|(target, title)| format!(r#"<li><a href="a.xhtml#{target}">{title}</a></li>"#))
        .collect::<String>();
    synthetic_xhtml(
        &format!(r#"<nav epub:type="toc"><ol>{list}</ol></nav>"#),
        "",
    )
}

#[test]
fn fragment_inside_emphasis_selects_the_whole_lexical_word() -> TestResult {
    let page = synthetic_xhtml(r#"<p>inter<em id="target">national</em> next</p>"#, "");
    let nav = synthetic_nav(&[("target", "International")]);
    let fixture = ModifiedFixture::new(&[
        ("EPUB/a.xhtml", page.as_bytes()),
        ("EPUB/nav.xhtml", nav.as_bytes()),
    ])?;
    let book = fixture.import()?;
    assert_eq!(book.chapters.len(), 1);
    let start = book
        .pieces
        .iter()
        .find(|p| p.id == book.chapters[0].start_piece)
        .ok_or("chapter start missing")?;
    assert_eq!(start.text, "international");
    assert_eq!(start.sources.len(), 2);
    verify_sources(&fixture.path, &book, &Limits::default())?;
    Ok(())
}

#[test]
fn hidden_and_manual_note_fragments_cannot_capture_later_visible_text() -> TestResult {
    let page = synthetic_xhtml(
        r#"<p hidden="hidden" id="hidden">Invisible</p><aside id="note" epub:type="footnote"><p>Manual aside.</p></aside><h1 id="real">Real chapter</h1><p>Visible later text.</p>"#,
        "",
    );
    let nav = synthetic_nav(&[
        ("hidden", "Hidden destination"),
        ("note", "Note destination"),
        ("real", "Real chapter"),
    ]);
    let fixture = ModifiedFixture::new(&[
        ("EPUB/a.xhtml", page.as_bytes()),
        ("EPUB/nav.xhtml", nav.as_bytes()),
    ])?;
    let book = fixture.import()?;
    assert_eq!(book.chapters.len(), 1);
    assert_eq!(book.chapters[0].id, "EPUB/a.xhtml#real");
    assert!(
        !book
            .pieces
            .iter()
            .any(|p| p.text.contains("Invisible") || p.text == "Manual")
    );
    assert!(
        book.diagnostics
            .iter()
            .filter(|s| s.contains("navigation target"))
            .count()
            >= 2
    );
    Ok(())
}

#[test]
fn duplicate_ids_preserve_content_and_emit_structured_warnings() -> TestResult {
    let page = synthetic_xhtml(r#"<h1 id="same">One</h1><p id="same">Two</p>"#, "");
    let fixture = ModifiedFixture::new(&[("EPUB/a.xhtml", page.as_bytes())])?;
    let original = std::fs::read(&fixture.path)?;
    let book = fixture.import()?;
    let text = block_text(&book).join(" ");
    assert!(text.contains("One"));
    assert!(text.contains("Two"));
    assert!(
        book.diagnostics
            .iter()
            .any(|d| d.contains("duplicate_xhtml_id") && d.contains("same"))
    );
    verify_sources(&fixture.path, &book, &Limits::default())?;
    assert_eq!(std::fs::read(&fixture.path)?, original);
    Ok(())
}

#[test]
fn opening_errors_identify_malformed_document_and_truncated_archive() -> TestResult {
    let page = synthetic_xhtml("<p>Broken</div>", "");
    let fixture = ModifiedFixture::new(&[("EPUB/a.xhtml", page.as_bytes())])?;
    let error = fixture
        .import()
        .err()
        .ok_or("malformed markup accepted")?
        .to_string();
    assert!(error.contains("malformed XML/HTML in EPUB/a.xhtml"));
    assert!(error.contains("p") && error.contains("div") && error.contains(" at "));
    let bytes = std::fs::read(&fixture.path)?;
    std::fs::write(&fixture.path, &bytes[..32])?;
    let error = fixture
        .import()
        .err()
        .ok_or("truncated ZIP accepted")?
        .to_string();
    assert!(error.contains("end-of-archive directory is missing"));
    Ok(())
}

#[test]
fn semantic_and_named_title_pages_remain_readable_without_chapter_pauses() -> TestResult {
    let page = synthetic_xhtml(
        r#"<section epub:type="titlepage" id="semantic"><p>Semantic front page.</p></section><h1 id="named">Named front page</h1><h1 id="real">Actual chapter</h1><p>Main passage.</p>"#,
        "",
    );
    let nav = synthetic_nav(&[
        ("semantic", "Opening"),
        ("named", "Title page"),
        ("real", "Actual chapter"),
    ]);
    let fixture = ModifiedFixture::new(&[
        ("EPUB/a.xhtml", page.as_bytes()),
        ("EPUB/nav.xhtml", nav.as_bytes()),
    ])?;
    let book = fixture.import()?;
    assert_eq!(book.chapters.len(), 1);
    assert_eq!(book.chapters[0].id, "EPUB/a.xhtml#real");
    let text = block_text(&book).join(" ");
    assert!(text.contains("Semantic front page."));
    assert!(text.contains("Named front page"));
    Ok(())
}

#[test]
fn publisher_css_and_missing_layout_never_block_content() -> TestResult {
    let body = r#"<p class="screen" style="font: nonsense; display:none">Readable passage.</p><p hidden="hidden">HTML_SECRET</p><p><em>Emphasis</em> <strong>Strong</strong></p>"#;
    let head = r#"<style media="screen and (min-width: 500px)">@font-face { broken } p::before { content: 'CSS_SECRET'; } p { display:none; }</style><link rel="stylesheet" href="missing.css"/><link rel="stylesheet" type="application/adobe-page-template+xml" href="missing.xpgt"/>"#;
    let page = synthetic_xhtml(body, head);
    let fixture = ModifiedFixture::new(&[("EPUB/a.xhtml", page.as_bytes())])?;
    let book = fixture.import()?;
    let text = block_text(&book).join(" ");
    assert!(text.contains("Readable passage."));
    assert!(!text.contains("HTML_SECRET"));
    assert!(!text.contains("CSS_SECRET"));
    let runs: Vec<_> = book
        .documents
        .iter()
        .flat_map(|d| &d.blocks)
        .flat_map(|b| &b.runs)
        .collect();
    assert!(runs.iter().any(|r| r.text == "Emphasis" && r.emphasis));
    assert!(runs.iter().any(|r| r.text == "Strong" && r.strong));
    verify_sources(&fixture.path, &book, &Limits::default())?;
    Ok(())
}

#[test]
fn doctypes_allow_content_without_resolving_external_entities() -> TestResult {
    for declaration in [
        r#"<!DOCTYPE html>"#,
        r#"<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.1//EN" "http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd">"#,
        r#"<!DOCTYPE html [<!ENTITY word "Readable">]>"#,
    ] {
        let body = if declaration.contains("ENTITY") {
            "<p>&word; passage.</p>"
        } else {
            "<p>Readable passage.</p>"
        };
        let page = format!("{declaration}{}", synthetic_xhtml(body, ""));
        let fixture = ModifiedFixture::new(&[("EPUB/a.xhtml", page.as_bytes())])?;
        let book = fixture.import()?;
        assert!(block_text(&book).join(" ").contains("Readable passage."));
        verify_sources(&fixture.path, &book, &Limits::default())?;
    }
    let page = format!(
        r#"<!DOCTYPE html [<!ENTITY secret SYSTEM "file:///etc/passwd">]>{}"#,
        synthetic_xhtml("<p>&secret;</p>", "")
    );
    let fixture = ModifiedFixture::new(&[("EPUB/a.xhtml", page.as_bytes())])?;
    assert!(fixture.import().is_err());
    let page = format!(
        r#"<!DOCTYPE html [<!ENTITY loop "&loop;">]>{}"#,
        synthetic_xhtml("<p>&loop;</p>", "")
    );
    let fixture = ModifiedFixture::new(&[("EPUB/a.xhtml", page.as_bytes())])?;
    assert!(fixture.import().is_err());
    Ok(())
}

#[test]
fn forged_zip_declared_size_cannot_bypass_actual_expansion_limit() -> TestResult {
    let payload = vec![b'x'; 100_000];
    let fixture = ModifiedFixture::new(&[("EPUB/large.bin", &payload)])?;
    let mut bytes = std::fs::read(&fixture.path)?;
    let mut patched = false;
    for offset in 0..bytes.len().saturating_sub(46) {
        if bytes.get(offset..offset + 4) != Some(b"PK\x01\x02") {
            continue;
        }
        let name_len = u16::from_le_bytes([bytes[offset + 28], bytes[offset + 29]]) as usize;
        if bytes.get(offset + 46..offset + 46 + name_len) == Some(b"EPUB/large.bin") {
            bytes[offset + 24..offset + 28].copy_from_slice(&1u32.to_le_bytes());
            patched = true;
            break;
        }
    }
    assert!(patched, "test archive central directory was not patched");
    std::fs::write(&fixture.path, bytes)?;
    let options = ImportOptions {
        limits: Limits {
            expanded_bytes: 30_000,
            ..Limits::default()
        },
    };
    assert!(matches!(
        import_epub(&fixture.path, "forged", &options),
        Err(Error::Limit(_))
    ));
    Ok(())
}

#[test]
fn astral_symbols_stay_inside_whole_whitespace_delimited_passages() -> TestResult {
    let xml = r#"<html xmlns="http://www.w3.org/1999/xhtml"><head/><body><p>A😀B 𝄞tone é.</p></body></html>"#;
    let fixture = ModifiedFixture::new(&[("EPUB/a.xhtml", xml.as_bytes())])?;
    let book = fixture.import()?;
    let words: Vec<_> = book
        .pieces
        .iter()
        .filter(|p| p.sources.iter().any(|s| s.document == "EPUB/a.xhtml"))
        .map(|p| p.text.as_str())
        .collect();
    assert_eq!(words, ["A😀B", "𝄞tone", "é."]);
    verify_sources(&fixture.path, &book, &ImportOptions::default().limits)?;
    Ok(())
}

#[test]
fn hyphenated_compounds_split_automatically_with_connected_beats() -> TestResult {
    let page = synthetic_xhtml(
        r#"<p id="one">state-<em>of</em>-the-art, well‐known non‑fiction COVID-19 2026-10-21 10-20 -5 word--word https://example.com/a-b a-b@example.com</p><p>well-known.</p>"#,
        "",
    );
    let fixture = ModifiedFixture::new(&[("EPUB/a.xhtml", page.as_bytes())])?;
    let book = fixture.import()?;
    let first = &book.documents[0].blocks[0].id;
    let pieces: Vec<_> = book
        .pieces
        .iter()
        .filter(|p| &p.block_id == first)
        .collect();
    assert_eq!(
        pieces.iter().map(|p| p.text.as_str()).collect::<Vec<_>>(),
        [
            "state-",
            "of-",
            "the-",
            "art,",
            "well‐",
            "known",
            "non‑",
            "fiction",
            "COVID-",
            "19",
            "2026-10-21",
            "10-20",
            "-5",
            "word--",
            "word",
            "https://example.com/a-b",
            "a-b@example.com"
        ]
    );
    assert!(pieces[..4].iter().all(|p| p.word_id == pieces[0].word_id));
    assert!(pieces[..3].iter().all(|p| !p.paragraph_end));
    assert!(block_text(&book)[0].starts_with("state-of-the-art,"));
    verify_sources(&fixture.path, &book, &Limits::default())?;
    let plan = reader_core::session::plan(&book.pieces, &book.chapters, false);
    for (at, piece) in book.pieces.iter().enumerate() {
        let compiled = &plan[at];
        let action = reader_core::session::playback(&book, at, false).ok_or("no action")?;
        assert_eq!(compiled.multiplier, action.multiplier);
        assert_eq!(compiled.extra_ms, action.extra_ms);
        if matches!(piece.text.as_str(), "state-" | "of-" | "the-" | "well-") {
            assert_eq!(reader_core::session::delay_ms(&book, at, 600), 80);
        }
        if piece.text == "art," {
            assert_eq!(reader_core::session::delay_ms(&book, at, 600), 125);
        }
        if piece.text == "known." {
            assert_eq!(reader_core::session::delay_ms(&book, at, 600), 290);
        }
    }
    let cached = reader_core::session::book_from_value(serde_json::to_value(&book)?)?;
    assert_eq!(
        serde_json::to_value(&cached)?,
        serde_json::to_value(&book)?,
        "loading already split books must be idempotent"
    );
    Ok(())
}

#[test]
fn dash_joined_words_are_separate_playback_pieces() -> TestResult {
    let page = synthetic_xhtml(
        "<p>well-known well–known word—word word--word word——word é-lan word—‘quoted’ word’—next</p>",
        "",
    );
    let fixture = ModifiedFixture::new(&[("EPUB/a.xhtml", page.as_bytes())])?;
    let book = fixture.import()?;
    let first = &book.documents[0].blocks[0].id;
    let text: Vec<_> = book
        .pieces
        .iter()
        .filter(|p| &p.block_id == first)
        .map(|p| p.text.as_str())
        .collect();
    assert_eq!(
        text,
        [
            "well-",
            "known",
            "well–",
            "known",
            "word—",
            "word",
            "word--",
            "word",
            "word——",
            "word",
            "é-",
            "lan",
            "word—",
            "‘quoted’",
            "word’—",
            "next"
        ]
    );
    verify_sources(&fixture.path, &book, &Limits::default())?;
    Ok(())
}

#[test]
fn cached_dash_words_split_in_both_session_and_disk_readers() -> TestResult {
    let text = "well—known--phrase";
    let source =
        serde_json::json!({"document":"d", "node":"n", "offset":0, "length":text.chars().count()});
    let piece = serde_json::json!({"id":"p", "word_id":"w", "block_id":"b", "kind":"word",
        "text":text, "paragraph_end":true, "sources":[source.clone()]});
    let block = serde_json::json!({"id":"b", "kind":"paragraph", "runs":[{"text":text, "sources":[source]}]});
    let mut old = serde_json::json!({"schema_version":1, "book_id":"dash-cache", "revision":"v1", "title":"Dashes",
        "documents":[{"id":"d", "blocks":[block.clone()]}], "pieces":[piece.clone()],
        "chapters":[{"id":"c", "title":"Chapter", "start_piece":"p"}],
        "notes":[{"id":"note", "blocks":[block], "pieces":[piece]}]});
    old["pieces"][0]["note_id"] = serde_json::json!("note");
    let unsplit: Book = serde_json::from_value(old.clone())?;
    let old_saved = reader_core::location::SavedLocation::capture(&unsplit, "p")?;
    let book = reader_core::session::book_from_value(old.clone())?;
    let expected = ["well—", "known--", "phrase"];
    assert_eq!(
        book.pieces
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(
        book.notes[0]
            .pieces
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(
        book.pieces
            .iter()
            .map(|p| p.sources[0].offset)
            .collect::<Vec<_>>(),
        [0, 5, 12]
    );
    assert_eq!(
        book.pieces
            .iter()
            .map(|p| p.paragraph_end)
            .collect::<Vec<_>>(),
        [false, false, true]
    );
    assert_eq!(
        reader_core::session::playback(&book, 0, false)
            .ok_or("missing playback")?
            .multiplier,
        1.0
    );
    assert_eq!(
        reader_core::session::playback(&book, 1, false)
            .ok_or("missing playback")?
            .multiplier,
        1.0
    );
    let normalized = serde_json::to_value(&book)?;
    assert_eq!(
        serde_json::to_value(reader_core::session::book_from_value(normalized.clone())?)?,
        normalized
    );
    let root = std::env::temp_dir().join(format!("reader-dash-cache-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root)?;
    let path = root.join("book.json");
    std::fs::write(&path, serde_json::to_vec(&old)?)?;
    let disk = reader_core::book_store::DiskBook::open(&path)?;
    let window = disk.load_window(0)?;
    assert_eq!(window.total, 3);
    assert_eq!(
        serde_json::to_value(&window.book.pieces)?,
        serde_json::to_value(&book.pieces)?
    );
    assert_eq!(
        serde_json::to_value(&window.book.notes)?,
        serde_json::to_value(&book.notes)?
    );
    assert!(matches!(
        recover_location(&window.book, &old_saved),
        Recovery::Exact { .. }
    ));
    let saved = reader_core::location::SavedLocation::capture(&book, &book.pieces[1].id)?;
    assert!(matches!(
        recover_location(&window.book, &saved),
        Recovery::Exact { .. }
    ));
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn streaming_export_matches_normalization_and_preserves_failed_output() -> TestResult {
    let root = std::env::temp_dir().join(format!("reader-export-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root)?;
    let output = root.join("new/nested/book.json");
    for name in [
        "complex.epub",
        "legacy.epub",
        "revised.epub",
        "unrelated.epub",
    ] {
        let path = fixture(name);
        let expected = import_epub(&path, "streamed-book", &ImportOptions::default())?;
        let summary =
            reader_core::export_epub(&path, "streamed-book", &ImportOptions::default(), &output)?;
        let actual: Book = serde_json::from_reader(std::fs::File::open(&output)?)?;
        assert_eq!(
            serde_json::to_value(&actual)?,
            serde_json::to_value(&expected)?
        );
        assert_eq!(summary.pieces, expected.pieces.len());
        assert_eq!(summary.documents, expected.documents.len());
        assert_eq!(summary.notes, expected.notes.len());
        assert_eq!(summary.chapters, expected.chapters.len());
        verify_sources(&path, &actual, &Limits::default())?;
    }
    let before = std::fs::read(&output)?;
    let broken = root.join("broken.epub");
    std::fs::write(&broken, b"not an EPUB")?;
    assert!(
        reader_core::export_epub(&broken, "streamed-book", &ImportOptions::default(), &output)
            .is_err()
    );
    assert_eq!(std::fs::read(&output)?, before);
    assert_eq!(std::fs::read_dir(&root)?.count(), 2);
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn streaming_long_epub_reads_last_window_without_collecting_book() -> TestResult {
    use std::fmt::Write as _;
    use std::io::Write;
    let root = std::env::temp_dir().join(format!("reader-long-export-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root)?;
    let epub = root.join("long.epub");
    let output = root.join("book.json");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&epub)?);
    let zip_options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file("mimetype", zip_options)?;
    zip.write_all(b"application/epub+zip")?;
    zip.start_file("META-INF/container.xml", zip_options)?;
    zip.write_all(
        br#"<container><rootfiles><rootfile full-path="package.opf"/></rootfiles></container>"#,
    )?;
    let mut package = String::from(
        r#"<package><metadata><title>Long synthetic publication</title></metadata><manifest>"#,
    );
    let chapters = 128;
    let words_per_chapter = 192;
    for chapter in 0..chapters {
        write!(
            package,
            r#"<item id="c{chapter}" href="c{chapter}.xhtml" media-type="application/xhtml+xml"/>"#
        )?;
    }
    package.push_str("</manifest><spine>");
    for chapter in 0..chapters {
        write!(package, r#"<itemref idref="c{chapter}"/>"#)?;
    }
    package.push_str("</spine></package>");
    zip.start_file("package.opf", zip_options)?;
    zip.write_all(package.as_bytes())?;
    let word = "a".repeat(511);
    for chapter in 0..chapters {
        zip.start_file(format!("c{chapter}.xhtml"), zip_options)?;
        zip.write_all(b"<html><head/><body><p>")?;
        for _ in 0..words_per_chapter {
            write!(zip, "{word} ")?;
        }
        zip.write_all(b"</p></body></html>")?;
    }
    zip.finish()?;
    assert!(std::fs::metadata(&epub)?.len() > 10 * 1024 * 1024);
    let summary =
        reader_core::export_epub(&epub, "long-synthetic", &ImportOptions::default(), &output)?;
    assert_eq!(summary.documents, chapters);
    assert_eq!(summary.pieces, chapters * words_per_chapter);
    let disk = reader_core::book_store::DiskBook::open(&output)?;
    assert_eq!(disk.total_pieces(), summary.pieces);
    let last = disk.load_window(summary.pieces - 1)?;
    assert!(last.book.pieces.len() <= reader_core::book_store::WINDOW_PIECES);
    assert_eq!(disk.piece(summary.pieces - 1)?.text, word);
    drop(disk);
    std::fs::remove_dir_all(root)?;
    Ok(())
}
