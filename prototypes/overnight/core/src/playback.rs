//! Shared playback interruption policy. Plans contain no timers, rendering state, or
//! automatic note traversal; frontends decide when a user resumes after a pause.
use crate::{Book, Piece, SourceSpan};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Step {
    Word {
        piece_id: String,
    },
    PauseImage {
        piece_id: String,
    },
    PauseUnsupported {
        piece_id: String,
    },
    ChapterBoundary {
        chapter_id: String,
        start_piece: String,
    },
    End,
}

/// An explicit note excursion. After its End, restore `return_piece` in the main
/// document. None means the reference was the final main piece. The return target
/// is the immediate next main piece, even when that is another manual note marker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteVisit {
    pub note_id: String,
    pub steps: Vec<Step>,
    pub return_piece: Option<String>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PlaybackError {
    #[error("main note reference was not found or is ambiguous")]
    InvalidReference,
    #[error("piece is not a resolved note reference")]
    NotANote,
    #[error("note target was not found or is ambiguous")]
    InvalidNoteTarget,
    #[error("source position was not found")]
    MissingSource,
    #[error("source position corresponds to multiple pieces")]
    AmbiguousSource,
}

/// Produces main-book actions in reading order. Chapter boundaries are determined
/// by logical chapters, not document changes. The initial chapter needs no pause;
/// subsequent chapters emit a boundary immediately before their first piece.
/// Note references and proven ornaments are skipped. Every other unknown kind
/// pauses as unsupported, so a new importer kind cannot disappear accidentally.
pub fn main_plan(book: &Book) -> Vec<Step> {
    let mut plan = Vec::new();
    for piece in &book.pieces {
        for chapter in book
            .chapters
            .iter()
            .skip(1)
            .filter(|c| c.start_piece == piece.id)
        {
            plan.push(Step::ChapterBoundary {
                chapter_id: chapter.id.clone(),
                start_piece: piece.id.clone(),
            });
        }
        if let Some(step) = piece_step(piece) {
            plan.push(step);
        }
    }
    plan.push(Step::End);
    plan
}

/// Plans one user-requested note visit. Nested note references are manual and are
/// skipped in this plan; broken references remain unsupported pauses. The API
/// accepts only a main-book reference, so it cannot recursively traverse notes.
pub fn visit_note(book: &Book, reference_piece_id: &str) -> Result<NoteVisit, PlaybackError> {
    let mut references = book
        .pieces
        .iter()
        .enumerate()
        .filter(|(_, p)| p.id == reference_piece_id);
    let (index, reference) = match (references.next(), references.next()) {
        (Some(found), None) => found,
        _ => return Err(PlaybackError::InvalidReference),
    };
    if reference.kind != "note" {
        return Err(PlaybackError::NotANote);
    }
    let id = reference.note_id.as_ref().ok_or(PlaybackError::NotANote)?;
    let mut notes = book.notes.iter().filter(|note| &note.id == id);
    let note = match (notes.next(), notes.next()) {
        (Some(note), None) => note,
        _ => return Err(PlaybackError::InvalidNoteTarget),
    };
    let mut steps: Vec<_> = note.pieces.iter().filter_map(piece_step).collect();
    steps.push(Step::End);
    Ok(NoteVisit {
        note_id: id.clone(),
        steps,
        return_piece: book.pieces.get(index + 1).map(|p| p.id.clone()),
    })
}

/// Finds the unique main or note piece containing a clicked Unicode scalar.
/// `position.offset` is the clicked scalar; `position.length` is ignored. Match
/// requires the same document and text-node identity. Callers must use the Book's
/// revision and convert UTF-16 offsets before calling this function.
pub fn piece_at_source<'a>(
    book: &'a Book,
    position: &SourceSpan,
) -> Result<&'a str, PlaybackError> {
    let mut pieces = book
        .pieces
        .iter()
        .chain(book.notes.iter().flat_map(|note| &note.pieces))
        .filter(|piece| {
            piece.sources.iter().any(|source| {
                source.document == position.document
                    && source.node == position.node
                    && source.offset <= position.offset
                    && source
                        .offset
                        .checked_add(source.length)
                        .is_some_and(|end| position.offset < end)
            })
        });
    match (pieces.next(), pieces.next()) {
        (Some(piece), None) => Ok(&piece.id),
        (None, _) => Err(PlaybackError::MissingSource),
        (Some(_), Some(_)) => Err(PlaybackError::AmbiguousSource),
    }
}

fn piece_step(piece: &Piece) -> Option<Step> {
    match piece.kind.as_str() {
        "note" | "ornament" => None,
        "word" => Some(Step::Word {
            piece_id: piece.id.clone(),
        }),
        "image" => Some(Step::PauseImage {
            piece_id: piece.id.clone(),
        }),
        _ => Some(Step::PauseUnsupported {
            piece_id: piece.id.clone(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ImportOptions, import_epub};
    use std::path::Path;

    fn fixture() -> crate::Result<Book> {
        import_epub(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/complex.epub"),
            "synthetic-local",
            &ImportOptions::default(),
        )
    }

    #[test]
    fn imported_note_returns_to_immediate_main_piece() -> Result<(), Box<dyn std::error::Error>> {
        let book = fixture()?;
        let (index, reference) = book
            .pieces
            .iter()
            .enumerate()
            .find(|(_, p)| p.kind == "note")
            .ok_or("fixture must contain a resolved note")?;
        let visit = visit_note(&book, &reference.id)?;
        assert_eq!(
            visit.return_piece.as_deref(),
            book.pieces.get(index + 1).map(|p| p.id.as_str())
        );
        assert!(matches!(visit.steps.last(), Some(Step::End)));
        assert!(
            visit
                .steps
                .iter()
                .any(|step| matches!(step, Step::Word { .. }))
        );
        let main = main_plan(&book);
        assert!(
            !main
                .iter()
                .any(|step| matches!(step, Step::Word { piece_id } if piece_id == &reference.id))
        );
        Ok(())
    }

    #[test]
    fn imported_images_unsupported_and_logical_chapters_pause()
    -> Result<(), Box<dyn std::error::Error>> {
        let book = fixture()?;
        let plan = main_plan(&book);
        assert!(
            plan.iter()
                .any(|step| matches!(step, Step::PauseImage { .. }))
        );
        assert!(
            plan.iter()
                .any(|step| matches!(step, Step::PauseUnsupported { .. }))
        );
        assert_eq!(
            plan.iter()
                .filter(|step| matches!(step, Step::ChapterBoundary { .. }))
                .count(),
            book.chapters.len().saturating_sub(1)
        );
        for ornament in book.pieces.iter().filter(|p| p.kind == "ornament") {
            assert!(!plan.iter().any(
                |step| matches!(step, Step::PauseImage { piece_id } if piece_id == &ornament.id)
            ));
        }
        Ok(())
    }

    #[test]
    fn source_click_inside_inline_emphasis_selects_whole_word()
    -> Result<(), Box<dyn std::error::Error>> {
        let book = fixture()?;
        let word = book
            .pieces
            .iter()
            .find(|p| p.kind == "word" && p.sources.len() > 1)
            .ok_or("fixture must contain a word across inline emphasis")?;
        for source in &word.sources {
            assert_eq!(piece_at_source(&book, source)?, word.id);
        }
        let mut missing = word.sources[0].clone();
        missing.offset = usize::MAX;
        assert_eq!(
            piece_at_source(&book, &missing),
            Err(PlaybackError::MissingSource)
        );
        Ok(())
    }

    #[test]
    fn nested_note_references_do_not_recurse() -> Result<(), Box<dyn std::error::Error>> {
        let mut book = fixture()?;
        let reference = book
            .pieces
            .iter()
            .find(|p| p.kind == "note")
            .ok_or("missing note")?
            .clone();
        let note = book
            .notes
            .iter_mut()
            .find(|n| Some(&n.id) == reference.note_id.as_ref())
            .ok_or("missing target")?;
        let mut recursive = reference.clone();
        recursive.id = "synthetic-recursive-note-marker".into();
        note.pieces.push(recursive.clone());
        let visit = visit_note(&book, &reference.id)?;
        assert!(
            !visit
                .steps
                .iter()
                .any(|step| matches!(step, Step::Word { piece_id } if piece_id == &recursive.id))
        );
        assert_eq!(
            visit_note(&book, &recursive.id),
            Err(PlaybackError::InvalidReference)
        );
        Ok(())
    }
}
