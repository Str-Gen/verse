//! Source-aware, bounded EPUB normalization for two reader frontends.
//!
//! Source offsets count Unicode scalar values in decoded XML text nodes, not UTF-16 units.
//! Import failures return without modifying a caller's current book. Output is reader data,
//! never executable publisher HTML. See the component README for the CSS support boundary.
mod archive;
mod css;
pub mod location;
mod normalize;
mod ornaments;
pub mod playback;

pub use archive::Limits;
pub use normalize::{import_epub, inspect_rbook, verify_sources};
use serde::{Deserialize, Serialize};

/// An address in decoded source XML. Valid only for its book revision.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceSpan {
    pub document: String,
    pub node: String,
    pub offset: usize,
    pub length: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Run {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note_id: Option<String>,
    pub text: String,
    pub emphasis: bool,
    pub strong: bool,
    pub sources: Vec<SourceSpan>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Block {
    pub id: String,
    pub kind: String,
    pub runs: Vec<Run>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub blocks: Vec<Block>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Piece {
    pub id: String,
    pub text: String,
    pub word_id: String,
    pub sources: Vec<SourceSpan>,
    pub block_id: String,
    pub paragraph_end: bool,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Chapter {
    /// Stable destination, including fragment, rather than its array position.
    pub id: String,
    pub title: String,
    pub start_piece: String,
    pub end_piece_exclusive: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub blocks: Vec<Block>,
    pub pieces: Vec<Piece>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageEvidence {
    pub resource: String,
    pub width: u32,
    pub height: u32,
    pub exact_occurrences: usize,
    pub mirrored_or_identical_assets: usize,
    pub classification: String,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Resource {
    pub id: String,
    pub media_type: String,
    /// Bounded embedded raster data. SVG is never embedded as executable content.
    pub data_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Book {
    pub schema_version: u32,
    pub book_id: String,
    pub revision: String,
    pub title: String,
    pub publisher_identifiers: Vec<String>,
    pub documents: Vec<Document>,
    pub pieces: Vec<Piece>,
    pub chapters: Vec<Chapter>,
    pub notes: Vec<Note>,
    pub resources: Vec<Resource>,
    pub image_evidence: Vec<ImageEvidence>,
    pub diagnostics: Vec<String>,
}

/// Controls playback presentation without changing lexical recognition or source locations.
#[derive(Clone, Debug, Default)]
pub struct ImportOptions {
    pub limits: Limits,
    /// Only explicitly approved compounds are split, at hyphens, into display pieces.
    pub split_compounds: std::collections::BTreeSet<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O failure: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid ZIP archive: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("resource limit exceeded: {0}")]
    Limit(&'static str),
    #[error("invalid EPUB: {0}")]
    Invalid(String),
    #[error("unsupported CSS: {0}")]
    Css(String),
}

pub type Result<T> = std::result::Result<T, Error>;
