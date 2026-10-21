//! Application policy shared by native shells. Timers and rendering stay in the shell.
use crate::{
    Book, Piece,
    location::{LocationError, Recovery, SavedLocation, recover_location},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Adapts early version-1 frontend fixtures to the core contract. Importer exports
/// already carry explicit kinds. Missing kinds inherit their containing block;
/// unknown blocks remain unsupported instead of turning into words.
pub fn book_from_value(mut value: Value) -> Result<Book, serde_json::Error> {
    let kinds: std::collections::HashMap<String, String> = value["documents"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|d| d["blocks"].as_array().into_iter().flatten())
        .filter_map(|b| Some((b["id"].as_str()?.to_owned(), b["kind"].as_str()?.to_owned())))
        .collect();
    if value
        .get("pieces")
        .and_then(Value::as_array)
        .is_some_and(|pieces| pieces.iter().any(|p| !p.is_object()))
    {
        return serde_json::from_value(value);
    }
    if let Some(pieces) = value.get_mut("pieces").and_then(Value::as_array_mut) {
        for piece in pieces {
            if piece.get("kind").is_none() {
                let kind = piece["block_id"]
                    .as_str()
                    .and_then(|id| kinds.get(id))
                    .map(String::as_str)
                    .unwrap_or("unsupported");
                piece["kind"] = json!(match kind {
                    "paragraph" | "heading" | "list_item" => "word",
                    other => other,
                });
            }
            if piece.get("word_id").is_some_and(Value::is_null) {
                piece["word_id"] = json!("");
            }
        }
    }
    let mut book = serde_json::from_value(value)?;
    crate::compound::split_book(&mut book).map_err(serde::de::Error::custom)?;
    Ok(book)
}

pub const MIN_SPEED: u32 = 60;
pub const MAX_SPEED: u32 = 1200;
pub fn speed(value: u32) -> u32 {
    value.clamp(MIN_SPEED, MAX_SPEED)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Word,
    Image,
    Unsupported,
    Note,
    Ornament,
}
pub fn kind(piece: &Piece) -> Kind {
    match piece.kind.as_str() {
        "word" => Kind::Word,
        "image" => Kind::Image,
        "note" => Kind::Note,
        "ornament" => Kind::Ornament,
        _ => Kind::Unsupported,
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Playback {
    pub kind: Kind,
    pub next: Option<usize>,
    pub chapter_boundary: bool,
    pub multiplier: f64,
    pub extra_ms: u32,
}
/// Resolves the next displayed piece and chapter interruption, including boundaries
/// whose first piece is a skipped note or ornament. End retains the last display.
pub fn playback(book: &Book, at: usize, follow_notes: bool) -> Option<Playback> {
    let piece = book.pieces.get(at)?;
    let next = (at + 1..book.pieces.len()).find(|&i| !skip(&book.pieces[i], follow_notes));
    let chapter_boundary = next.is_some_and(|next| {
        book.chapters.iter().skip(1).any(|c| {
            book.pieces[at + 1..=next]
                .iter()
                .any(|p| p.id == c.start_piece)
        })
    });
    Some(action(
        piece,
        next,
        chapter_boundary,
        crate::compound::continues(piece, book.pieces.get(at + 1)),
    ))
}
fn action(
    piece: &Piece,
    next: Option<usize>,
    chapter_boundary: bool,
    continues_compound: bool,
) -> Playback {
    let ending = piece.text.trim_end_matches(['”', '\'', '"', '’', ')']);
    let multiplier = if continues_compound {
        0.8
    } else if ending.ends_with(['.', '!', '?']) {
        1.7
    } else if ending.ends_with([',', ';', ':']) {
        1.25
    } else {
        1.0
    };
    Playback {
        kind: kind(piece),
        next,
        chapter_boundary,
        multiplier,
        extra_ms: if piece.paragraph_end { 120 } else { 0 },
    }
}
/// Compiles shell actions in linear time, including long runs of skipped markers.
/// Detached notes pass no chapters and disable following nested notes.
pub fn plan(pieces: &[Piece], chapters: &[crate::Chapter], follow_notes: bool) -> Vec<Playback> {
    let starts: std::collections::HashSet<_> = chapters
        .iter()
        .skip(1)
        .map(|c| c.start_piece.as_str())
        .collect();
    let mut next = None;
    let mut boundary = false;
    let mut result = Vec::with_capacity(pieces.len());
    for (at, piece) in pieces.iter().enumerate().rev() {
        result.push(action(
            piece,
            next,
            next.is_some() && boundary,
            crate::compound::continues(piece, pieces.get(at + 1)),
        ));
        if !skip(piece, follow_notes) {
            next = Some(at);
            boundary = false;
        }
        boundary |= starts.contains(piece.id.as_str());
    }
    result.reverse();
    result
}

pub fn skip(piece: &Piece, follow_notes: bool) -> bool {
    kind(piece) == Kind::Ornament || (kind(piece) == Kind::Note && !follow_notes)
}
pub fn start(book: &Book, at: usize, follow_notes: bool) -> Option<usize> {
    (at..book.pieces.len()).find(|&i| !skip(&book.pieces[i], follow_notes))
}
pub fn delay_ms(book: &Book, at: usize, wpm: u32) -> u32 {
    playback(book, at, false).map_or(0, |p| {
        (60_000.0 / f64::from(speed(wpm)) * p.multiplier).round() as u32 + p.extra_ms
    })
}

#[derive(Debug, Serialize)]
pub struct Resume {
    pub kind: String,
    pub at: usize,
}
/// Reads new anchors and conservatively migrates either legacy frontend record.
/// Changed revisions propose the saved chapter start without matching passages.
/// An unchanged revision can migrate only a unique exact source anchor. No write occurs here.
pub fn restore(book: &Book, saved: Option<&Value>) -> Result<Resume, LocationError> {
    let Some(saved) = saved else {
        return Ok(Resume {
            kind: "fresh".into(),
            at: 0,
        });
    };
    if !saved.is_object()
        || !saved
            .get("revision")
            .and_then(Value::as_str)
            .is_some_and(|r| !r.is_empty())
    {
        return Err(LocationError::InvalidLocation(
            "saved record has no valid revision".into(),
        ));
    }
    let location = if let Some(location) = saved.get("location") {
        serde_json::from_value::<SavedLocation>(location.clone())?
    } else {
        let revision = saved
            .get("revision")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let sources: Vec<crate::SourceSpan> =
            serde_json::from_value(saved.get("sources").cloned().unwrap_or(json!([])))?;
        let mut exact = book
            .pieces
            .iter()
            .filter(|p| !sources.is_empty() && p.sources == sources);
        let text = match (revision == book.revision).then(|| (exact.next(), exact.next())) {
            Some((Some(p), None)) => p.text.clone(),
            _ => String::new(),
        };
        let chapter = saved.get("chapter");
        // Legacy Tauri records used a chapter ID string.
        serde_json::from_value(json!({
            "book_id": book.book_id, "revision": revision, "sources": sources,
            "text": text, "context": [], "context_anchor": 0,
            "chapter_id": chapter.and_then(|c| c.as_str().or_else(|| c.get("id").and_then(Value::as_str))),
            "chapter_title": chapter.and_then(|c| c.get("title")).and_then(Value::as_str)
        }))?
    };
    // Changed books resume at an identified chapter, without scanning passage text.
    if location.revision != book.revision {
        let mut chapters = book.chapters.iter().filter(|chapter| {
            location.book_id == book.book_id && location.chapter_id.as_ref() == Some(&chapter.id)
        });
        let at = match (chapters.next(), chapters.next()) {
            (Some(chapter), None) => book.pieces.iter().position(|p| p.id == chapter.start_piece),
            _ => None,
        };
        return Ok(Resume {
            kind: if at.is_some() {
                "chapter"
            } else {
                "unresolved"
            }
            .into(),
            at: at.unwrap_or(0),
        });
    }
    let (kind, id) = match recover_location(book, &location) {
        Recovery::Exact { piece_id } => ("exact", Some(piece_id)),
        Recovery::ProposePassage { piece_id } => ("passage", Some(piece_id)),
        Recovery::ProposeChapter { piece_id, .. } => ("chapter", Some(piece_id)),
        Recovery::Preserve { .. } => ("unresolved", None),
    };
    Ok(Resume {
        kind: kind.into(),
        at: id
            .and_then(|id| book.pieces.iter().position(|p| p.id == id))
            .unwrap_or(0),
    })
}
/// Captures only a durable main-text anchor; caller preferences are preserved.
/// The caller must resolve any pending recovery before requesting a save.
pub fn capture(book: &Book, at: usize, preferences: &Value) -> Result<Value, LocationError> {
    let piece = book
        .pieces
        .get(at)
        .filter(|p| !p.sources.is_empty())
        .ok_or_else(|| {
            LocationError::InvalidLocation("current passage has no source anchor".into())
        })?;
    let location = SavedLocation::capture(book, &piece.id)?;
    let mut record = preferences.as_object().cloned().unwrap_or_default();
    record.insert("revision".into(), json!(book.revision));
    record.insert("sources".into(), json!(piece.sources));
    record.insert("location".into(), serde_json::to_value(location)?);
    let wpm = record
        .get("speed")
        .and_then(Value::as_u64)
        .and_then(|n| u32::try_from(n).ok())
        .unwrap_or(350);
    record.insert("speed".into(), json!(speed(wpm)));
    record.remove("context");
    record.remove("chapter");
    Ok(Value::Object(record))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn book() -> Result<Book, serde_json::Error> {
        serde_json::from_value(
            json!({"schema_version":1,"book_id":"local", "revision":"one", "title":"Synthetic", "documents":[],
            "pieces": "alpha bravo charlie delta echo foxtrot golf hotel india juliet kilo lima mike november oscar papa".split_whitespace().enumerate().map(|(i,text)| json!({
                "id":format!("p{i}"),"word_id":format!("w{i}"),"kind":"word","text":text,"block_id":"b",
                "sources":[{"document":"d","node":format!("n{i}"),"offset":0,"length":text.len()}]
            })).collect::<Vec<_>>(), "chapters":[{"id":"d#one","title":"One","start_piece":"p0"}]}),
        )
    }
    #[test]
    fn resume_is_stopped_policy_and_changed_editions_only_propose()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut book = book()?;
        let saved = capture(&book, 5, &json!({"speed":425,"history":["kept"]}))?;
        assert_eq!(restore(&book, Some(&saved))?.kind, "exact");
        assert_eq!(restore(&book, Some(&saved))?.at, 5);
        assert_eq!(saved["history"], json!(["kept"]));
        let before = saved.clone();
        book.revision = "two".into();
        assert_eq!(restore(&book, Some(&saved))?.kind, "chapter");
        assert_eq!(restore(&book, Some(&saved))?.at, 0);
        assert_eq!(saved, before);
        book.book_id = "other".into();
        assert_eq!(restore(&book, Some(&saved))?.kind, "unresolved");
        Ok(())
    }
    #[test]
    fn both_legacy_formats_require_stronger_evidence_after_revision_change()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut book = book()?;
        for legacy in [
            json!({"revision":"one","sources":book.pieces[5].sources,"context":{"pieces":[["alpha","word"],["bravo","word"],["charlie","word"]],"selected_offset":1},"chapter":{"id":"d#one","title":"One"}}),
            json!({"revision":"one","sources":book.pieces[5].sources,"context":"alpha\u{1f}bravo\u{1f}charlie","chapter":"d#one"}),
        ] {
            assert_eq!(restore(&book, Some(&legacy))?.kind, "exact");
            book.revision = "two".into();
            assert_ne!(restore(&book, Some(&legacy))?.kind, "passage");
            book.revision = "one".into();
        }
        Ok(())
    }
    #[test]
    fn short_or_repeated_context_only_offers_identified_chapter()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut book = book()?;
        book.pieces.truncate(7);
        let saved = capture(&book, 2, &json!({}))?;
        book.revision = "two".into();
        assert_eq!(restore(&book, Some(&saved))?.kind, "chapter");
        book.chapters[0].id = "different".into();
        assert_eq!(restore(&book, Some(&saved))?.kind, "unresolved");
        Ok(())
    }
    #[test]
    fn skipped_markers_do_not_hide_chapters_or_unknown_content()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut book = book()?;
        book.pieces[1].kind = "ornament".into();
        book.pieces[2].kind = "note".into();
        book.pieces[3].kind = "future-media".into();
        let mut chapter = book.chapters[0].clone();
        chapter.id = "two".into();
        chapter.start_piece = "p1".into();
        book.chapters.push(chapter);
        let step = playback(&book, 0, false).ok_or("step")?;
        assert_eq!(step.next, Some(3));
        assert!(step.chapter_boundary);
        assert_eq!(
            playback(&book, 3, false).ok_or("step")?.kind,
            Kind::Unsupported
        );
        assert_eq!(playback(&book, 0, true).ok_or("step")?.next, Some(2));
        assert_eq!(start(&book, 1, false), Some(3));
        book.pieces[15].kind = "ornament".into();
        assert_eq!(playback(&book, 14, false).ok_or("step")?.next, None);
        Ok(())
    }
    #[test]
    fn compiled_actions_preserve_chapters_and_note_interruptions()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut book = book()?;
        book.pieces[1].kind = "ornament".into();
        book.pieces[2].kind = "note".into();
        book.pieces[3].kind = "image".into();
        let mut chapter = book.chapters[0].clone();
        chapter.start_piece = "p1".into();
        book.chapters.push(chapter);
        let plan = plan(&book.pieces, &book.chapters, false);
        assert_eq!(plan[0].next, Some(3));
        assert!(plan[0].chapter_boundary);
        assert_eq!(plan[3].kind, Kind::Image);
        assert!(!plan[3].chapter_boundary);
        assert_eq!(plan.last().and_then(|p| p.next), None);
        Ok(())
    }
    #[test]
    fn legacy_adapter_preserves_note_kinds_and_rejects_malformed_pieces()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut value = serde_json::to_value(book()?)?;
        value["documents"] = json!([{"id":"d","blocks":[{"id":"b","kind":"note","runs":[]}]}]);
        value["pieces"][0]
            .as_object_mut()
            .ok_or("piece")?
            .remove("kind");
        value["pieces"][0]["word_id"] = Value::Null;
        assert_eq!(kind(&book_from_value(value.clone())?.pieces[0]), Kind::Note);
        value["pieces"][0] = json!("bad piece");
        assert!(book_from_value(value).is_err());
        assert!(book_from_value(json!(42)).is_err());
        assert!(restore(&book()?, Some(&json!({"speed":425}))).is_err());
        assert!(restore(&book()?, Some(&json!(42))).is_err());
        Ok(())
    }
    #[test]
    fn private_state_round_trip_and_failed_replacement_preserve_bytes()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory =
            std::env::temp_dir().join(format!("reader-shared-{}", uuid::Uuid::new_v4()));
        let path = directory.join("resume.json");
        let book = book()?;
        let saved = capture(&book, 5, &json!({"speed":425}))?;
        crate::location::write_json(&path, &saved)?;
        let before = std::fs::read(&path)?;
        let loaded = crate::location::read_json(&path)?;
        assert_eq!(restore(&book, Some(&loaded))?.at, 5);
        assert!(crate::location::write_json_with_limit(&path, &json!("too large"), 2).is_err());
        assert_eq!(std::fs::read(&path)?, before);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path)?.permissions().mode() & 0o777,
                0o600
            );
        }
        std::fs::write(&path, b"{broken")?;
        assert!(crate::location::read_json::<Value>(&path).is_err());
        assert_eq!(std::fs::read(&path)?, b"{broken");
        std::fs::remove_dir_all(directory)?;
        Ok(())
    }
    #[test]
    fn punctuation_and_speed_limits_are_shared() -> Result<(), Box<dyn std::error::Error>> {
        let mut book = book()?;
        assert_eq!(delay_ms(&book, 0, 600), 100);
        book.pieces[0].text = "Finished.)".into();
        book.pieces[0].paragraph_end = true;
        assert_eq!(delay_ms(&book, 0, 600), 290);
        assert_eq!(speed(0), 60);
        assert_eq!(speed(u32::MAX), 1200);
        book.pieces[0].sources.clear();
        assert!(capture(&book, 0, &json!({})).is_err());
        Ok(())
    }
}
