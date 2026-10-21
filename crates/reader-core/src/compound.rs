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

pub(crate) fn split_pieces(pieces: &mut Vec<Piece>) -> Result<()> {
    let mut ids: HashSet<_> = pieces.iter().map(|p| p.id.clone()).collect();
    let mut result = Vec::with_capacity(pieces.len());
    for piece in std::mem::take(pieces) {
        if piece.kind != "word"
            || !piece.text.contains(separator)
            || piece.text.contains(['/', '\\', '@'])
            || !piece.text.chars().any(char::is_alphabetic)
        {
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
