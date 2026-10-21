//! Playback beats retain the original lexical identity and exact source spans.
use crate::{Book, Error, Piece, Result};
use std::collections::HashSet;

fn hyphen(character: char) -> bool {
    matches!(character, '-' | '\u{2010}' | '\u{2011}')
}

pub(crate) fn continues(piece: &Piece, next: Option<&Piece>) -> bool {
    piece.kind == "word"
        && !piece.word_id.is_empty()
        && piece.text.ends_with(hyphen)
        && next.is_some_and(|next| {
            next.kind == "word" && next.word_id == piece.word_id && next.block_id == piece.block_id
        })
}

pub(crate) fn split_book(book: &mut Book) -> Result<()> {
    split(&mut book.pieces)?;
    for note in &mut book.notes {
        split(&mut note.pieces)?;
    }
    Ok(())
}

fn split(pieces: &mut Vec<Piece>) -> Result<()> {
    let mut ids: HashSet<_> = pieces.iter().map(|p| p.id.clone()).collect();
    let mut result = Vec::with_capacity(pieces.len());
    for piece in std::mem::take(pieces) {
        if piece.kind != "word"
            || !piece.text.contains(hyphen)
            || piece.text.contains(['/', '\\', '@'])
            || !piece.text.chars().any(char::is_alphabetic)
        {
            result.push(piece);
            continue;
        }
        let chars: Vec<_> = piece.text.chars().collect();
        let mut cuts = vec![0];
        for (index, triple) in chars.windows(3).enumerate() {
            if triple[0].is_alphanumeric() && hyphen(triple[1]) && triple[2].is_alphanumeric() {
                cuts.push(index + 2);
            }
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
