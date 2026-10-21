//! Playback beats retain the original lexical identity and exact source spans.
use crate::{Book, Error, Piece, Result};
use std::collections::HashSet;

fn hyphen(character: char) -> bool {
    matches!(character, '-' | '\u{2010}' | '\u{2011}')
}

fn separator(character: char) -> bool {
    hyphen(character)
        || matches!(
            character,
            '\u{2012}'
                ..='\u{2015}' | '\u{2e3a}' | '\u{2e3b}' | '\u{fe58}' | '\u{fe63}' | '\u{ff0d}'
        )
}

pub(crate) fn continues(piece: &Piece, next: Option<&Piece>) -> bool {
    piece.kind == "word"
        && !piece.word_id.is_empty()
        && piece.text.ends_with(hyphen)
        && !piece.text.chars().rev().nth(1).is_some_and(separator)
        && next.is_some_and(|next| {
            next.kind == "word" && next.word_id == piece.word_id && next.block_id == piece.block_id
        })
}

pub(crate) fn split_book(book: &mut Book) -> Result<()> {
    split_pieces(&mut book.pieces)?;
    for note in &mut book.notes {
        split_pieces(&mut note.pieces)?;
    }
    Ok(())
}

fn may_split(piece: &Piece) -> bool {
    piece.kind == "word"
        && piece.text.contains(separator)
        && !piece.text.contains(['/', '\\', '@'])
        && piece.text.chars().any(char::is_alphabetic)
}

pub(crate) fn split_pieces(pieces: &mut Vec<Piece>) -> Result<()> {
    if !pieces.iter().any(may_split) {
        return Ok(());
    }
    let mut ids: HashSet<_> = pieces.iter().map(|p| p.id.clone()).collect();
    let mut result = Vec::with_capacity(pieces.len());
    for piece in std::mem::take(pieces) {
        if !may_split(&piece) {
            result.push(piece);
            continue;
        }
        let chars: Vec<_> = piece.text.chars().collect();
        let mut cuts = vec![0];
        let mut index = 0;
        let mut segment_start = 0;
        while index < chars.len() {
            if !separator(chars[index]) {
                index += 1;
                continue;
            }
            let start = index;
            while index < chars.len() && separator(chars[index]) {
                index += 1;
            }
            let left_word = chars[segment_start..start]
                .iter()
                .any(|c| c.is_alphanumeric());
            let right_word = chars[index..]
                .iter()
                .take_while(|c| !separator(**c))
                .any(|c| c.is_alphanumeric());
            if left_word && right_word {
                cuts.push(index);
            }
            segment_start = index;
        }
        if cuts.len() == 1 {
            result.push(piece);
            continue;
        }
        cuts.push(chars.len());
        let mut source_ranges = Vec::new();
        let mut offset = 0usize;
        for source in &piece.sources {
            let end = offset
                .checked_add(source.length)
                .ok_or(Error::Limit("compound source length"))?;
            source
                .offset
                .checked_add(source.length)
                .ok_or(Error::Limit("compound source offset"))?;
            source_ranges.push((offset..end, source));
            offset = end;
        }
        if offset != chars.len() {
            return Err(Error::Invalid(
                "compound text disagrees with its source spans".into(),
            ));
        }
        for (part, range) in cuts.windows(2).enumerate() {
            let mut beat = piece.clone();
            if part > 0 {
                let mut suffix = part;
                loop {
                    beat.id = format!("{}:hyphen:{suffix}", piece.id);
                    if ids.insert(beat.id.clone()) {
                        break;
                    }
                    suffix += 1;
                }
            }
            if beat.word_id.is_empty() {
                beat.word_id = piece.id.clone();
            }
            beat.text = chars[range[0]..range[1]].iter().collect();
            beat.paragraph_end = piece.paragraph_end && range[1] == chars.len();
            beat.sources = source_ranges
                .iter()
                .filter_map(|(source_range, source)| {
                    let start = range[0].max(source_range.start);
                    let end = range[1].min(source_range.end);
                    if start >= end {
                        return None;
                    }
                    let mut span = (*source).clone();
                    span.offset += start - source_range.start;
                    span.length = end - start;
                    Some(span)
                })
                .collect();
            result.push(beat);
        }
    }
    *pieces = result;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SourceSpan;

    fn piece(id: &str, text: &str) -> Piece {
        Piece {
            id: id.into(),
            text: text.into(),
            word_id: String::new(),
            sources: vec![SourceSpan {
                document: "chapter.xhtml".into(),
                node: "text-1".into(),
                offset: 7,
                length: text.chars().count(),
            }],
            block_id: "paragraph-1".into(),
            paragraph_end: true,
            kind: "word".into(),
            note_id: None,
        }
    }

    #[test]
    fn leaves_unsplit_pieces_unchanged() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut pieces = vec![
            piece("plain", "ordinary"),
            piece("unicode", "café中文"),
            piece("date", "2026-10-21"),
            piece("url", "https://example.com/a-b"),
            piece("email", "a-b@example.com"),
            piece("path", "dir\\a-b"),
            piece("symbol", "---"),
            piece("image", "well-known"),
        ];
        pieces[7].kind = "image".into();
        let before = serde_json::to_value(&pieces)?;
        split_pieces(&mut pieces)?;
        assert_eq!(serde_json::to_value(&pieces)?, before);
        split_pieces(&mut Vec::new())?;
        Ok(())
    }

    #[test]
    fn split_ids_reserve_both_earlier_and_later_unsplit_ids()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut pieces = vec![
            piece("root:hyphen:1", "before"),
            piece("root", "café—東京-test"),
            piece("root:hyphen:2", "after"),
        ];
        pieces[1].word_id = "lexical-word".into();
        split_pieces(&mut pieces)?;
        assert_eq!(
            pieces.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
            [
                "root:hyphen:1",
                "root",
                "root:hyphen:3",
                "root:hyphen:4",
                "root:hyphen:2"
            ]
        );
        assert_eq!(
            pieces.iter().map(|p| p.text.as_str()).collect::<Vec<_>>(),
            ["before", "café—", "東京-", "test", "after"]
        );
        assert!(pieces[1..4].iter().all(|p| p.word_id == "lexical-word"));
        assert_eq!(
            pieces[1..4]
                .iter()
                .map(|p| (p.sources[0].offset, p.sources[0].length, p.paragraph_end))
                .collect::<Vec<_>>(),
            [(7, 5, false), (12, 3, false), (15, 4, true)]
        );
        let before = serde_json::to_value(&pieces)?;
        split_pieces(&mut pieces)?;
        assert_eq!(serde_json::to_value(&pieces)?, before);
        Ok(())
    }
}
