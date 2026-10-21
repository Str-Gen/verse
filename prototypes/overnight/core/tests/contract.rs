use reader_core::location::{LocalStore, Recovery, recover_location};
use reader_core::{Book, Error, ImportOptions, Limits, import_epub, inspect_rbook, verify_sources};
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
fn visible_text_preserves_unicode_inline_words_and_supported_visibility() -> TestResult {
    let book = load("complex.epub")?;
    let visible = block_text(&book);
    assert!(
        visible
            .iter()
            .any(|s| s == "Café naïveté cooperate élan 👩‍🔬.")
    );
    assert!(visible.iter().any(|s| s == "A visible sentence."));
    assert!(visible.iter().any(|s| s == "Restored child."));
    assert!(visible.iter().any(|s| s == "Cascade visible."));
    let exported = serde_json::to_string(&book)?;
    for hidden in [
        "DISPLAY_SECRET",
        "VISIBILITY_SECRET",
        "HTML_SECRET",
        "DESCENDANT_SECRET",
    ] {
        assert!(!exported.contains(hidden), "hidden text leaked: {hidden}");
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
fn epub2_and_epub3_agree_on_reader_content_and_candidate_reads_both() -> TestResult {
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
    for name in ["complex.epub", "legacy.epub"] {
        assert_eq!(inspect_rbook(&fixture(name), &Limits::default())?, 3);
        let book = load(name)?;
        verify_sources(&fixture(name), &book, &Limits::default())?;
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
            total_image_pixels: 10,
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
            &ImportOptions {
                limits,
                ..ImportOptions::default()
            },
        );
        assert!(
            matches!(result, Err(Error::Limit(_) | Error::Invalid(_))),
            "resource limit {index} did not reject"
        );
    }
    Ok(())
}

#[test]
fn display_splitting_keeps_lexical_identity_and_changed_editions_only_propose() -> TestResult {
    let original = load("complex.epub")?;
    let compound = original
        .pieces
        .iter()
        .find(|p| p.text == "state-of-the-art")
        .ok_or("whole compound absent")?;
    let saved = reader_core::location::SavedLocation::capture(&original, &compound.id)?;
    let mut options = ImportOptions::default();
    options.split_compounds.insert("state-of-the-art".into());
    let split = import_epub(&fixture("complex.epub"), &original.book_id, &options)?;
    let displayed: Vec<_> = split
        .pieces
        .iter()
        .filter(|p| p.word_id == compound.word_id)
        .collect();
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
    verify_sources(&fixture("complex.epub"), &split, &options.limits)?;
    let revised = import_epub(
        &fixture("revised.epub"),
        &original.book_id,
        &ImportOptions::default(),
    )?;
    assert_ne!(revised.revision, original.revision);
    assert!(matches!(
        recover_location(&revised, &saved),
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
fn duplicate_fragment_identifiers_are_rejected() -> TestResult {
    let page = synthetic_xhtml(r#"<h1 id="same">One</h1><p id="same">Two</p>"#, "");
    let fixture = ModifiedFixture::new(&[("EPUB/a.xhtml", page.as_bytes())])?;
    assert!(matches!(fixture.import(), Err(Error::Invalid(_))));
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
fn stylesheet_media_is_screen_specific_and_unknown_queries_are_errors() -> TestResult {
    let body = r#"<p class="print">Print-visible passage.</p><p class="screen">SCREEN_SECRET</p>"#;
    let head = r#"<style media="print">.print { display:none; }</style><link rel="stylesheet" media="screen" href="media.css"/>"#;
    let page = synthetic_xhtml(body, head);
    let fixture = ModifiedFixture::new(&[
        ("EPUB/a.xhtml", page.as_bytes()),
        ("EPUB/media.css", b".screen { display:none; }"),
    ])?;
    let book = fixture.import()?;
    let exported = serde_json::to_string(&book)?;
    assert!(exported.contains("Print-visible passage."));
    assert!(!exported.contains("SCREEN_SECRET"));
    let unsupported = synthetic_xhtml(
        body,
        r#"<style media="screen and (min-width: 500px)">p { display:none; }</style>"#,
    );
    let variant = ModifiedFixture::new(&[("EPUB/a.xhtml", unsupported.as_bytes())])?;
    assert!(matches!(variant.import(), Err(Error::Css(_))));
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
        ..ImportOptions::default()
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
