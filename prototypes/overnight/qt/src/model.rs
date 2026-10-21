use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct Source {
    document: String,
    node: String,
    offset: usize,
    length: usize,
}
#[derive(Deserialize)]
struct Run {
    note_id: Option<String>,
    text: String,
    #[serde(default)]
    emphasis: bool,
    #[serde(default)]
    strong: bool,
    #[serde(default)]
    sources: Vec<Source>,
}
#[derive(Deserialize)]
struct Block {
    id: String,
    kind: String,
    #[serde(default)]
    runs: Vec<Run>,
    level: Option<u8>,
    alt: Option<String>,
    resource: Option<String>,
}
#[derive(Deserialize)]
struct Document {
    blocks: Vec<Block>,
}
#[derive(Deserialize)]
struct Piece {
    id: String,
    text: String,
    #[serde(default = "word_kind")]
    kind: String,
    block_id: String,
    #[serde(default)]
    sources: Vec<Source>,
}
fn word_kind() -> String {
    "word".into()
}
#[derive(Deserialize)]
struct Chapter {
    #[serde(default)]
    id: String,
    #[serde(default)]
    title: String,
    start_piece: String,
}
#[derive(Deserialize)]
struct Note {
    id: String,
    blocks: Vec<Block>,
}
#[derive(Deserialize)]
struct Resource {
    id: String,
    data_url: String,
}
#[derive(Deserialize)]
struct Book {
    schema_version: u32,
    book_id: String,
    revision: String,
    title: String,
    documents: Vec<Document>,
    pieces: Vec<Piece>,
    #[serde(default)]
    chapters: Vec<Chapter>,
    #[serde(default)]
    notes: Vec<Note>,
    #[serde(default)]
    resources: Vec<Resource>,
}
#[derive(Serialize, Deserialize)]
struct Saved {
    revision: String,
    sources: Vec<Source>,
    speed: u32,
    #[serde(default)]
    context: Option<SavedContext>,
    #[serde(default)]
    chapter: Option<SavedChapter>,
}
#[derive(Serialize, Deserialize)]
struct SavedContext {
    pieces: Vec<(String, String)>,
    selected_offset: usize,
}
#[derive(Serialize, Deserialize)]
struct SavedChapter {
    id: String,
    title: String,
}
struct Recovery {
    position: usize,
    description: String,
}
#[derive(Default, Serialize, Deserialize)]
struct SavedBooks {
    books: BTreeMap<String, Saved>,
}

pub struct Model {
    pub title: String,
    pub html: String,
    pub plain_text: String,
    pub current: usize,
    pub playing: bool,
    pub speed: u32,
    pub status: String,
    book: Book,
    positions: Vec<(usize, usize)>,
    preserve_saved: bool,
    inspected_stop: Option<usize>,
    recovery: Option<Recovery>,
}

impl Model {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let json = fs::read_to_string(path).map_err(|e| format!("Cannot read book JSON: {e}"))?;
        Self::from_json(&json, path.parent().unwrap_or_else(|| Path::new(".")))
    }

    fn from_json(json: &str, directory: &Path) -> Result<Self, String> {
        let book: Book =
            serde_json::from_str(json).map_err(|e| format!("Invalid book JSON: {e}"))?;
        if book.schema_version != 1 {
            return Err("Only book JSON schema version 1 is supported".into());
        }
        if book.pieces.is_empty() {
            return Err("Book has no playback pieces".into());
        }
        validate(&book)?;
        let (html, plain_text, positions) = render(&book, directory)?;
        Ok(Self {
            title: book.title.clone(),
            html,
            plain_text,
            current: 0,
            playing: false,
            speed: 350,
            status: "Stopped".into(),
            book,
            positions,
            preserve_saved: false,
            inspected_stop: None,
            recovery: None,
        })
    }

    pub fn note_text(&self, id: &str) -> String {
        self.book
            .notes
            .iter()
            .find(|note| note.id == id)
            .map_or_else(
                || "This note is unavailable.".into(),
                |note| {
                    note.blocks
                        .iter()
                        .map(|block| {
                            block
                                .runs
                                .iter()
                                .map(|run| run.text.as_str())
                                .collect::<String>()
                        })
                        .collect::<Vec<_>>()
                        .join("\n\n")
                },
            )
    }

    pub fn book_id(&self) -> &str {
        &self.book.book_id
    }
    pub fn current_text(&self) -> String {
        self.book.pieces[self.current].text.clone()
    }
    pub fn current_utf16(&self) -> usize {
        self.positions[self.current].0
    }
    pub fn current_utf16_end(&self) -> usize {
        self.positions[self.current].1
    }

    pub fn select_utf16(&mut self, position: usize) {
        self.playing = false;
        self.current = self
            .positions
            .iter()
            .position(|&(start, end)| position >= start && position < end)
            .or_else(|| {
                self.positions
                    .iter()
                    .enumerate()
                    .filter(|(_, (start, _))| *start >= position)
                    .min_by_key(|(_, (start, _))| *start)
                    .map(|(index, _)| index)
            })
            .unwrap_or(self.book.pieces.len() - 1);
        self.inspected_stop = None;
        self.status = "Selected passage".into();
    }

    pub fn start(&mut self) {
        if matches!(self.kind(), "image" | "unsupported") {
            if self.inspected_stop != Some(self.current) {
                self.stop_for_inspection();
                return;
            }
            if !self.advance() {
                return;
            }
        }
        if !self.skip_manual() {
            return;
        }
        if matches!(self.kind(), "image" | "unsupported") {
            self.stop_for_inspection();
        } else {
            self.playing = true;
            self.status = "Playing".into();
        }
    }

    fn stop_for_inspection(&mut self) {
        self.playing = false;
        self.inspected_stop = Some(self.current);
        self.status =
            "Image or unsupported content. Inspect in reading; press play to continue.".into();
    }

    pub fn pause(&mut self) {
        self.playing = false;
        self.status = "Stopped".into();
    }

    pub fn step(&mut self) {
        if !self.playing {
            return;
        }
        let last_displayed = self.current;
        if !self.advance() {
            return;
        }
        if !self.skip_manual() {
            self.current = last_displayed;
            return;
        }
        if self
            .book
            .chapters
            .iter()
            .skip(1)
            .any(|c| c.start_piece == self.book.pieces[self.current].id)
        {
            self.playing = false;
            self.status = "Chapter boundary. Press play to continue.".into();
        } else if matches!(self.kind(), "image" | "unsupported") {
            self.stop_for_inspection();
        }
    }

    fn kind(&self) -> &str {
        &self.book.pieces[self.current].kind
    }
    fn advance(&mut self) -> bool {
        if self.current + 1 == self.book.pieces.len() {
            self.playing = false;
            self.status = "End of book".into();
            false
        } else {
            self.current += 1;
            true
        }
    }
    fn skip_manual(&mut self) -> bool {
        while matches!(self.kind(), "note" | "ornament") {
            if !self.advance() {
                return false;
            }
        }
        true
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), String> {
        if self.preserve_saved {
            return Err("Saved position belongs to another revision; old state preserved".into());
        }
        if self.book.pieces[self.current].sources.is_empty() {
            return Err(
                "Current passage has no source spans; existing resume state preserved".into(),
            );
        }
        let path = path.as_ref();
        let mut saved = read_saved(path)?;
        saved.books.insert(
            self.book.book_id.clone(),
            Saved {
                revision: self.book.revision.clone(),
                sources: self.book.pieces[self.current].sources.clone(),
                speed: self.speed.clamp(60, 1200),
                context: Some(self.saved_context()),
                chapter: self.saved_chapter(),
            },
        );
        let json = serde_json::to_vec_pretty(&saved).map_err(|e| e.to_string())?;
        if let Some(parent) = path.parent() {
            let mut directory = fs::DirBuilder::new();
            directory.recursive(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                directory.mode(0o700);
            }
            directory.create(parent).map_err(|e| e.to_string())?;
        }
        let temporary = path.with_extension("pending");
        let mut options = fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temporary)
            .map_err(|e| format!("Cannot open resume state: {e}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(0o600))
                .map_err(|e| e.to_string())?;
        }
        use std::io::Write;
        file.write_all(&json)
            .map_err(|e| format!("Cannot write resume state: {e}"))?;
        file.sync_all()
            .map_err(|e| format!("Cannot flush resume state: {e}"))?;
        drop(file);
        fs::rename(temporary, path).map_err(|e| format!("Cannot commit resume state: {e}"))
    }

    fn saved_context(&self) -> SavedContext {
        let start = self.current.saturating_sub(3);
        let end = (self.current + 4).min(self.book.pieces.len());
        SavedContext {
            pieces: self.book.pieces[start..end]
                .iter()
                .map(|piece| (piece.text.clone(), piece.kind.clone()))
                .collect(),
            selected_offset: self.current - start,
        }
    }

    fn saved_chapter(&self) -> Option<SavedChapter> {
        let positions: BTreeMap<_, _> = self
            .book
            .pieces
            .iter()
            .enumerate()
            .map(|(index, piece)| (piece.id.as_str(), index))
            .collect();
        self.book
            .chapters
            .iter()
            .filter_map(|chapter| {
                positions
                    .get(chapter.start_piece.as_str())
                    .map(|index| (*index, chapter))
            })
            .filter(|(index, _)| *index <= self.current)
            .max_by_key(|(index, _)| *index)
            .map(|(_, chapter)| SavedChapter {
                id: chapter.id.clone(),
                title: chapter.title.clone(),
            })
    }

    fn find_recovery(&self, saved: &Saved) -> Option<Recovery> {
        if let Some(context) = &saved.context
            && context.pieces.len() >= 3
            && context.selected_offset < context.pieces.len()
        {
            let mut matches = self
                .book
                .pieces
                .windows(context.pieces.len())
                .enumerate()
                .filter(|(_, window)| {
                    window
                        .iter()
                        .zip(&context.pieces)
                        .all(|(piece, (text, kind))| piece.text == *text && piece.kind == *kind)
                });
            if let Some((start, _)) = matches.next()
                && matches.next().is_none()
            {
                return Some(Recovery { position: start + context.selected_offset,
                            description: "A unique matching passage was found in this edition. Confirm to resume there, stopped.".into() });
            }
        }
        let saved_chapter = saved.chapter.as_ref()?;
        let exact: Vec<_> = self
            .book
            .chapters
            .iter()
            .filter(|chapter| {
                !saved_chapter.id.is_empty()
                    && chapter.id == saved_chapter.id
                    && (saved_chapter.title.is_empty() || chapter.title == saved_chapter.title)
            })
            .collect();
        let titles: Vec<_> = self
            .book
            .chapters
            .iter()
            .filter(|chapter| {
                !saved_chapter.title.is_empty() && chapter.title == saved_chapter.title
            })
            .collect();
        let chapter = match exact.as_slice() {
            [chapter] => *chapter,
            _ => match titles.as_slice() {
                [chapter] => *chapter,
                _ => return None,
            },
        };
        let position = self
            .book
            .pieces
            .iter()
            .position(|piece| piece.id == chapter.start_piece)?;
        Some(Recovery { position, description: "The saved chapter was identified, but the passage could not be matched uniquely. Confirm to resume at that chapter's start, stopped.".into() })
    }

    pub fn recovery_description(&self) -> Option<String> {
        self.recovery.as_ref().map(|recovery| {
            let start = recovery.position.saturating_sub(3);
            let end = (recovery.position + 4).min(self.book.pieces.len());
            let preview = self.book.pieces[start..end]
                .iter()
                .map(|piece| piece.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            format!("{}\n\n{}", recovery.description, preview)
        })
    }

    pub fn needs_recovery(&self) -> bool {
        self.preserve_saved
    }

    pub fn confirm_current_location(&mut self) -> Result<(), String> {
        if !self.preserve_saved {
            return Err("No saved location requires replacement".into());
        }
        if self.book.pieces[self.current].sources.is_empty() {
            return Err("Selected location has no durable source spans".into());
        }
        self.recovery = None;
        self.preserve_saved = false;
        self.playing = false;
        self.inspected_stop = None;
        self.status = "Selected source location confirmed, stopped".into();
        Ok(())
    }

    pub fn confirm_recovery(&mut self) -> Result<(), String> {
        let recovery = self
            .recovery
            .take()
            .ok_or("No recovery is available to confirm")?;
        self.current = recovery.position;
        self.playing = false;
        self.inspected_stop = None;
        self.preserve_saved = false;
        self.status = "Recovered location confirmed, stopped".into();
        Ok(())
    }

    pub fn restore(&mut self, path: impl AsRef<Path>) -> Result<(), String> {
        self.playing = false;
        self.recovery = None;
        let saved = read_saved(path.as_ref())?;
        let Some(state) = saved.books.get(&self.book.book_id) else {
            return Ok(());
        };
        self.speed = state.speed.clamp(60, 1200);
        if state.revision != self.book.revision {
            self.preserve_saved = true;
            self.recovery = self.find_recovery(state);
            self.status = if self.recovery.is_some() {
                "Book revision changed. Recovery is available for confirmation; original state preserved."
            } else { "Book revision changed. No confident recovery found; original state preserved." }.into();
            return Err(self.status.clone());
        }
        let matches: Vec<_> = self
            .book
            .pieces
            .iter()
            .enumerate()
            .filter(|(_, piece)| !state.sources.is_empty() && piece.sources == state.sources)
            .map(|(index, _)| index)
            .collect();
        if let [index] = matches.as_slice() {
            self.current = *index;
            self.status = "Restored passage, stopped".into();
            Ok(())
        } else {
            self.preserve_saved = true;
            self.status =
                "Saved source location is missing or ambiguous; original state preserved".into();
            Err(self.status.clone())
        }
    }
}

fn read_saved(path: &Path) -> Result<SavedBooks, String> {
    match fs::read(path) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).map_err(|e| format!("Invalid saved state: {e}"))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(SavedBooks::default()),
        Err(e) => Err(format!("Cannot read saved state: {e}")),
    }
}
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn local_image_uri(resource: &str, directory: &Path) -> Result<String, String> {
    let path = if let Some(uri_path) = resource.strip_prefix("file:") {
        let path = if let Some(authority_and_path) = uri_path.strip_prefix("//") {
            if authority_and_path.starts_with('/') {
                authority_and_path
            } else if authority_and_path.starts_with("localhost/") {
                &authority_and_path["localhost".len()..]
            } else {
                return Err("Remote file URI authority is not allowed".into());
            }
        } else {
            uri_path
        };
        if !path.starts_with('/') || path.contains(['?', '#']) {
            return Err(
                "Image file URI must be an absolute local path without query or fragment".into(),
            );
        }
        let mut bytes = Vec::with_capacity(path.len());
        let mut input = path.bytes();
        while let Some(byte) = input.next() {
            if byte == b'%' {
                let high = input
                    .next()
                    .and_then(|b| char::from(b).to_digit(16))
                    .ok_or("Invalid file URI escape")?;
                let low = input
                    .next()
                    .and_then(|b| char::from(b).to_digit(16))
                    .ok_or("Invalid file URI escape")?;
                bytes.push((high * 16 + low) as u8);
            } else {
                bytes.push(byte);
            }
        }
        std::path::PathBuf::from(
            String::from_utf8(bytes).map_err(|_| "Image file URI is not UTF-8")?,
        )
    } else {
        if resource.starts_with("//")
            || resource
                .split('/')
                .next()
                .is_some_and(|part| part.contains(':'))
        {
            return Err("Only local image paths and file URIs are allowed".into());
        }
        directory.join(resource)
    };
    if path.as_os_str().to_string_lossy().contains('\0') {
        return Err("Image path contains NUL".into());
    }
    let absolute = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .map_err(|e| format!("Cannot resolve image directory: {e}"))?
            .join(path)
    };
    let path_text = absolute.to_str().ok_or("Image path is not UTF-8")?;
    let mut uri = String::from("file://");
    for byte in path_text.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~".contains(&byte) {
            uri.push(char::from(byte));
        } else {
            use std::fmt::Write;
            write!(&mut uri, "%{byte:02X}").map_err(|e| e.to_string())?;
        }
    }
    Ok(uri)
}

fn validate(book: &Book) -> Result<(), String> {
    let mut blocks = BTreeSet::new();
    for block in book
        .documents
        .iter()
        .flat_map(|doc| &doc.blocks)
        .chain(book.notes.iter().flat_map(|note| &note.blocks))
    {
        if !blocks.insert(block.id.as_str()) {
            return Err("Duplicate block ID".into());
        }
        if !matches!(
            block.kind.as_str(),
            "heading" | "paragraph" | "list_item" | "image" | "ornament" | "unsupported" | "note"
        ) {
            return Err(format!("Unsupported block kind: {}", block.kind));
        }
        for run in &block.runs {
            let mut length = 0usize;
            for source in &run.sources {
                source
                    .offset
                    .checked_add(source.length)
                    .ok_or("Source range overflows")?;
                length = length
                    .checked_add(source.length)
                    .ok_or("Run source lengths overflow")?;
            }
            if !run.sources.is_empty() && length != run.text.chars().count() {
                return Err("Run scalar length differs from its source spans".into());
            }
        }
    }
    let mut pieces = BTreeSet::new();
    for piece in &book.pieces {
        if !pieces.insert(piece.id.as_str()) {
            return Err("Duplicate piece ID".into());
        }
        if !blocks.contains(piece.block_id.as_str()) {
            return Err("Piece references unknown block".into());
        }
        if !matches!(
            piece.kind.as_str(),
            "word" | "image" | "unsupported" | "ornament" | "note"
        ) {
            return Err(format!("Unsupported piece kind: {}", piece.kind));
        }
        for source in &piece.sources {
            source
                .offset
                .checked_add(source.length)
                .ok_or("Source range overflows")?;
        }
    }
    for chapter in &book.chapters {
        if !pieces.contains(chapter.start_piece.as_str()) {
            return Err("Chapter references unknown piece".into());
        }
    }
    Ok(())
}

fn qt_plain_character(character: char) -> char {
    match character {
        '\r' | '\u{2028}' | '\u{2029}' => '\n',
        '\u{a0}' => ' ',
        other => other,
    }
}

type Rendered = (String, String, Vec<(usize, usize)>);
type NodePositions = BTreeMap<usize, (usize, usize)>;

fn render(book: &Book, directory: &Path) -> Result<Rendered, String> {
    let mut html = String::from(
        "<html><head><style>p { line-height: 140%; } h1, h2, h3, h4, h5, h6 { font-size: large; margin-top: 20px; margin-bottom: 12px; }</style></head><body>",
    );
    let mut plain = String::new();
    let mut source_positions: BTreeMap<(&str, &str), NodePositions> = BTreeMap::new();
    let mut blocks = BTreeMap::new();
    let mut utf16 = 0;
    for block in book.documents.iter().flat_map(|doc| &doc.blocks) {
        if matches!(block.kind.as_str(), "paragraph" | "heading" | "ornament")
            && block.runs.iter().all(|run| run.text.is_empty())
        {
            blocks.insert(block.id.as_str(), (utf16, utf16));
            continue;
        }
        if !plain.is_empty() {
            plain.push('\n');
            utf16 += 1;
        }
        let start = utf16;
        let tag = if block.kind == "heading" {
            format!("h{}", block.level.unwrap_or(2).clamp(1, 6))
        } else {
            "p".into()
        };
        html.push_str(&format!("<{tag} style=\"white-space: pre-wrap;\">"));
        if block.kind == "list_item" {
            html.push_str("• ");
            plain.push_str("• ");
            utf16 += 2;
        }
        if block.kind == "image" {
            if let Some(resource) = &block.resource {
                let uri = if let Some(embedded) = book.resources.iter().find(|r| &r.id == resource)
                {
                    if ![
                        "data:image/png;base64,",
                        "data:image/jpeg;base64,",
                        "data:image/webp;base64,",
                    ]
                    .iter()
                    .any(|prefix| embedded.data_url.starts_with(prefix))
                    {
                        return Err("Unsupported embedded image format".into());
                    }
                    embedded.data_url.clone()
                } else {
                    local_image_uri(resource, directory)?
                };
                html.push_str(&format!("<img src=\"{}\" />", escape(&uri)));
                plain.push('\u{fffc}');
                utf16 += 1;
            }
            if block.resource.is_none() {
                let caption = format!(
                    "[{}]",
                    block
                        .alt
                        .as_deref()
                        .filter(|alt| !alt.trim().is_empty())
                        .unwrap_or("Image unavailable")
                );
                html.push_str(&escape(&caption));
                plain.extend(caption.chars().map(qt_plain_character));
                utf16 += caption.encode_utf16().count();
            }
        }
        if block.kind == "unsupported" {
            let label = "[Unsupported content] ";
            html.push_str(label);
            plain.push_str(label);
            utf16 += label.len();
        }
        if block.kind == "note" {
            let label = "Note: ";
            html.push_str(label);
            plain.push_str(label);
            utf16 += label.len();
        }
        for run in &block.runs {
            if let Some(id) = &run.note_id {
                html.push_str(&format!("<a href=\"note:{}\">", escape(id)));
            }
            if run.emphasis {
                html.push_str("<i>");
            }
            if run.strong {
                html.push_str("<b>");
            }
            html.push_str(&escape(&run.text));
            if run.strong {
                html.push_str("</b>");
            }
            if run.emphasis {
                html.push_str("</i>");
            }
            if run.note_id.is_some() {
                html.push_str("</a>");
            }
            let mut source_index = 0;
            let mut source_offset = 0;
            for character in run.text.chars() {
                while run
                    .sources
                    .get(source_index)
                    .is_some_and(|s| source_offset >= s.length)
                {
                    source_offset = 0;
                    source_index += 1;
                }
                let next = utf16 + character.len_utf16();
                if let Some(source) = run.sources.get(source_index) {
                    let positions = source_positions
                        .entry((&source.document, &source.node))
                        .or_default();
                    if positions
                        .insert(source.offset + source_offset, (utf16, next))
                        .is_some()
                    {
                        return Err("A source scalar appears in multiple rendered locations".into());
                    }
                    source_offset += 1;
                }
                plain.push(qt_plain_character(character));
                utf16 = next;
            }
        }
        html.push_str(&format!("</{tag}>"));
        blocks.insert(block.id.as_str(), (start, utf16.max(start + 1)));
    }
    html.push_str("</body></html>");
    let positions = book
        .pieces
        .iter()
        .map(|piece| {
            let mut first = None;
            let mut last = None;
            for source in &piece.sources {
                let mut mapped_count = 0;
                if let Some(node) =
                    source_positions.get(&(source.document.as_str(), source.node.as_str()))
                {
                    for (_, &(start, end)) in
                        node.range(source.offset..source.offset + source.length)
                    {
                        first = Some(first.map_or(start, |value: usize| value.min(start)));
                        last = Some(last.map_or(end, |value: usize| value.max(end)));
                        mapped_count += 1;
                    }
                }
                if piece.kind == "word" && mapped_count != source.length {
                    return Err(format!("Piece {} has an unmappable source span", piece.id));
                }
            }
            match (first, last) {
                (Some(start), Some(end)) => Ok((start, end)),
                _ if piece.kind != "word" => blocks
                    .get(piece.block_id.as_str())
                    .copied()
                    .ok_or_else(|| format!("Piece {} has no rendered block", piece.id)),
                _ => Err(format!("Piece {} has no mapped source", piece.id)),
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok((html, plain, positions))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn demo() -> Result<Model, String> {
        Model::from_json(include_str!("../fixtures/demo.json"), Path::new("fixtures"))
    }
    #[test]
    fn utf16_selection_preserves_non_bmp_source() -> Result<(), Box<dyn std::error::Error>> {
        let mut model = demo()?;
        let scalar = model.plain_text.find("🦊").ok_or("fixture missing emoji")?;
        let position = model.plain_text[..scalar].encode_utf16().count();
        model.select_utf16(position + 1);
        assert_eq!(model.current_text(), "🦊");
        assert_eq!(model.current_utf16(), position);
        assert_eq!(model.current_utf16_end(), position + 2);
        model.start();
        model.step();
        model.pause();
        assert_eq!(model.current_text(), "crossed");
        Ok(())
    }
    #[test]
    fn playback_stops_at_image_and_chapter() -> Result<(), Box<dyn std::error::Error>> {
        let mut model = demo()?;
        model.start();
        for _ in 0..2000 {
            if !model.playing {
                break;
            }
            model.step();
        }
        assert_eq!(model.kind(), "image");
        model.start();
        for _ in 0..2000 {
            if !model.playing {
                break;
            }
            model.step();
        }
        assert!(model.status.contains("Chapter boundary"));
        Ok(())
    }
    #[test]
    fn resume_is_source_based_stopped_and_revision_safe() -> Result<(), Box<dyn std::error::Error>>
    {
        let path = std::env::temp_dir().join(format!("qt-model-test-{}.json", std::process::id()));
        let mut model = demo()?;
        model.current = 12;
        model.speed = 480;
        model.start();
        model.save(&path)?;
        let mut restored = demo()?;
        restored.restore(&path)?;
        assert_eq!(restored.current, 12);
        assert_eq!(restored.speed, 480);
        assert!(!restored.playing);
        let before = fs::read(&path)?;
        restored.book.revision = "changed".into();
        assert!(restored.restore(&path).is_err());
        assert!(restored.save(&path).is_err());
        assert_eq!(fs::read(&path)?, before);
        fs::remove_file(path)?;
        Ok(())
    }
    #[test]
    fn inline_split_word_keeps_all_sources() -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::json!({
            "schema_version": 1, "book_id": "split", "revision": "v1", "title": "Split",
            "documents": [{"id":"d", "blocks":[{"id":"b", "kind":"paragraph", "runs":[
                {"text":"🦊ca", "sources":[{"document":"d","node":"a","offset":0,"length":3}]},
                {"text":"fé", "emphasis":true,"sources":[{"document":"d","node":"b","offset":0,"length":2}]}
            ]}]}], "pieces":[
                {"id":"p1","text":"🦊","block_id":"b","sources":[{"document":"d","node":"a","offset":0,"length":1}]},
                {"id":"p2","text":"café","block_id":"b","sources":[
                    {"document":"d","node":"a","offset":1,"length":2},
                    {"document":"d","node":"b","offset":0,"length":2}]}]
        });
        let mut model = Model::from_json(&json.to_string(), Path::new("."))?;
        model.select_utf16(5);
        assert_eq!(model.current_text(), "café");
        assert_eq!((model.current_utf16(), model.current_utf16_end()), (2, 6));
        assert!(model.html.contains("<i>fé</i>"));
        Ok(())
    }

    #[test]
    fn malformed_mapping_fails_instead_of_jumping_to_start()
    -> Result<(), Box<dyn std::error::Error>> {
        let original: serde_json::Value =
            serde_json::from_str(include_str!("../fixtures/demo.json"))?;
        let mut missing = original.clone();
        missing["pieces"][0]["sources"][0]["node"] = "missing".into();
        assert!(Model::from_json(&missing.to_string(), Path::new(".")).is_err());
        let mut ambiguous = original.clone();
        let run = ambiguous["documents"][0]["blocks"][0]["runs"][0].clone();
        ambiguous["documents"][0]["blocks"][0]["runs"]
            .as_array_mut()
            .ok_or("runs")?
            .push(run);
        assert!(Model::from_json(&ambiguous.to_string(), Path::new(".")).is_err());
        let mut duplicate = original;
        duplicate["pieces"][1]["id"] = duplicate["pieces"][0]["id"].clone();
        assert!(Model::from_json(&duplicate.to_string(), Path::new(".")).is_err());
        Ok(())
    }

    #[test]
    fn state_is_isolated_per_book() -> Result<(), Box<dyn std::error::Error>> {
        let path =
            std::env::temp_dir().join(format!("qt-model-isolation-{}.json", std::process::id()));
        let mut first = demo()?;
        first.current = 5;
        first.speed = 400;
        first.save(&path)?;
        let mut second = demo()?;
        second.book.book_id = "another-book".into();
        second.current = 8;
        second.speed = 600;
        second.save(&path)?;
        first.current = 0;
        first.restore(&path)?;
        second.current = 0;
        second.restore(&path)?;
        assert_eq!((first.current, first.speed), (5, 400));
        assert_eq!((second.current, second.speed), (8, 600));
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn selected_image_requires_explicit_continue() -> Result<(), Box<dyn std::error::Error>> {
        let mut model = demo()?;
        let image = model
            .book
            .pieces
            .iter()
            .position(|p| p.kind == "image")
            .ok_or("image")?;
        model.select_utf16(model.positions[image].0);
        model.start();
        assert!(!model.playing);
        assert_eq!(model.current, image);
        model.start();
        assert!(model.playing);
        assert!(model.current > image);
        model.current = model.book.pieces.len() - 1;
        model.book.pieces[model.current].kind = "note".into();
        model.start();
        assert!(!model.playing);
        assert_eq!(model.status, "End of book");
        Ok(())
    }
    #[test]
    fn local_image_uris_are_encoded_without_remote_fetches() -> Result<(), String> {
        assert_eq!(
            local_image_uri("coast image.svg", Path::new("/tmp/book"))?,
            "file:///tmp/book/coast%20image.svg"
        );
        assert_eq!(
            local_image_uri("file:///tmp/caf%C3%A9%23image.svg", Path::new("/unused"))?,
            "file:///tmp/caf%C3%A9%23image.svg"
        );
        assert_eq!(
            local_image_uri("file://localhost/tmp/image.svg", Path::new("/unused"))?,
            "file:///tmp/image.svg"
        );
        for blocked in [
            "https://example.com/a.svg",
            "data:image/svg+xml,a",
            "file://server/a.svg",
            "//server/a.svg",
            "file:///tmp/%00.svg",
            "file:///tmp/%xy.svg",
        ] {
            assert!(local_image_uri(blocked, Path::new(".")).is_err());
        }
        Ok(())
    }

    #[test]
    fn qt_plain_text_whitespace_and_empty_blocks_keep_source_offsets()
    -> Result<(), Box<dyn std::error::Error>> {
        let text = "a  b\tc\nd\re\r\nf\u{a0}g\u{2028}h\u{2029}i";
        let source = serde_json::json!({"document":"d", "node":"n", "offset":0,"length":text.chars().count()});
        let json = serde_json::json!({"schema_version":1,"book_id":"space","revision":"1","title":"Whitespace",
            "documents":[{"id":"d","blocks":[
                {"id":"empty","kind":"paragraph","runs":[]},
                {"id":"b","kind":"paragraph","runs":[{"text":text,"sources":[source.clone()]}]},
                {"id":"empty-last","kind":"heading","runs":[]}]}],
            "pieces":[{"id":"p","text":text,"block_id":"b","sources":[source]}]});
        let model = Model::from_json(&json.to_string(), Path::new("."))?;
        assert_eq!(model.plain_text, "a  b\tc\nd\ne\n\nf g\nh\ni");
        assert_eq!(model.current_utf16(), 0);
        assert_eq!(model.current_utf16_end(), text.encode_utf16().count());
        assert_eq!(model.html.matches("<p ").count(), 1);
        Ok(())
    }

    #[test]
    fn image_resume_uses_sources_and_source_less_save_preserves_state()
    -> Result<(), Box<dyn std::error::Error>> {
        let path = std::env::temp_dir().join(format!("qt-model-image-{}.json", std::process::id()));
        let mut model = demo()?;
        model.current = model
            .book
            .pieces
            .iter()
            .position(|p| p.kind == "image")
            .ok_or("image")?;
        model.save(&path)?;
        let mut restored = demo()?;
        restored.restore(&path)?;
        assert_eq!(restored.current, model.current);
        assert!(!restored.playing);
        let before = fs::read(&path)?;
        model.book.pieces[model.current].sources.clear();
        assert!(model.save(&path).is_err());
        assert_eq!(fs::read(&path)?, before);
        fs::remove_file(path)?;
        Ok(())
    }
    #[test]
    fn changed_revision_requires_confirmation_before_unique_recovery()
    -> Result<(), Box<dyn std::error::Error>> {
        let path =
            std::env::temp_dir().join(format!("qt-model-recovery-{}.json", std::process::id()));
        let mut old = demo()?;
        old.current = 12;
        old.save(&path)?;
        let before = fs::read(&path)?;
        let mut revised = demo()?;
        revised.book.revision = "second".into();
        assert!(revised.restore(&path).is_err());
        assert_eq!(revised.current, 0);
        assert!(
            revised
                .recovery_description()
                .is_some_and(|description| description.contains("unique matching passage"))
        );
        assert!(revised.save(&path).is_err());
        assert_eq!(fs::read(&path)?, before);
        revised.confirm_recovery()?;
        assert_eq!(revised.current, 12);
        assert!(!revised.playing);
        revised.save(&path)?;
        let saved = read_saved(&path)?;
        assert_eq!(
            saved
                .books
                .get(revised.book_id())
                .ok_or("saved book")?
                .revision,
            "second"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&path)?.permissions().mode() & 0o777, 0o600);
        }
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn repeated_context_offers_chapter_start_without_overwriting()
    -> Result<(), Box<dyn std::error::Error>> {
        let path = std::env::temp_dir().join(format!(
            "qt-model-chapter-recovery-{}.json",
            std::process::id()
        ));
        let mut old = demo()?;
        for piece in &mut old.book.pieces {
            piece.text = "repeated".into();
            piece.kind = "word".into();
        }
        old.current = 15;
        old.save(&path)?;
        let mut revised = demo()?;
        revised.book.revision = "second".into();
        for piece in &mut revised.book.pieces {
            piece.text = "repeated".into();
            piece.kind = "word".into();
        }
        assert!(revised.restore(&path).is_err());
        assert!(
            revised
                .recovery_description()
                .is_some_and(|description| description.contains("chapter's start"))
        );
        assert!(revised.save(&path).is_err());
        revised.confirm_recovery()?;
        assert_eq!(revised.current, 0);
        assert!(!revised.playing);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn unrecoverable_and_legacy_revisions_preserve_old_state()
    -> Result<(), Box<dyn std::error::Error>> {
        let path =
            std::env::temp_dir().join(format!("qt-model-no-recovery-{}.json", std::process::id()));
        let mut old = demo()?;
        old.current = 12;
        old.save(&path)?;
        let before = fs::read(&path)?;
        let mut revised = demo()?;
        revised.book.revision = "second".into();
        for piece in &mut revised.book.pieces {
            piece.text = "different".into();
        }
        revised.book.chapters.clear();
        assert!(revised.restore(&path).is_err());
        assert!(revised.recovery_description().is_none());
        assert!(revised.confirm_recovery().is_err());
        assert!(revised.save(&path).is_err());
        assert_eq!(fs::read(&path)?, before);
        let mut legacy: serde_json::Value = serde_json::from_slice(&before)?;
        let record = legacy["books"][old.book_id()]
            .as_object_mut()
            .ok_or("record")?;
        record.remove("context");
        record.remove("chapter");
        fs::write(&path, serde_json::to_vec(&legacy)?)?;
        assert!(revised.restore(&path).is_err());
        assert!(revised.recovery_description().is_none());
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn detached_notes_cannot_disappear_silently() -> Result<(), Box<dyn std::error::Error>> {
        let mut json: serde_json::Value =
            serde_json::from_str(include_str!("../fixtures/demo.json"))?;
        json["notes"] = serde_json::json!([{"id":"detached","text":"Synthetic note"}]);
        assert!(Model::from_json(&json.to_string(), Path::new(".")).is_err());
        json["notes"] = serde_json::json!([{"id":"detached","blocks":[{
            "id":"detached-block","kind":"paragraph","runs":[{"text":"Synthetic note"}]
        }]}]);
        let model = Model::from_json(&json.to_string(), Path::new("."))?;
        assert!(!model.plain_text.contains("Synthetic note"));
        assert_eq!(model.note_text("detached"), "Synthetic note");
        assert_eq!(model.note_text("missing"), "This note is unavailable.");
        Ok(())
    }
    #[test]
    fn explicit_new_location_replaces_unrecoverable_state_only_after_confirmation()
    -> Result<(), Box<dyn std::error::Error>> {
        let path = std::env::temp_dir().join(format!(
            "qt-model-explicit-recovery-{}.json",
            std::process::id()
        ));
        let mut old = demo()?;
        old.current = 12;
        old.save(&path)?;
        let before = fs::read(&path)?;
        let mut revised = demo()?;
        revised.book.revision = "second".into();
        for piece in &mut revised.book.pieces {
            piece.text = "different".into();
        }
        revised.book.chapters.clear();
        assert!(revised.restore(&path).is_err());
        assert!(revised.needs_recovery());
        revised.select_utf16(revised.positions[20].0);
        assert!(revised.save(&path).is_err());
        assert_eq!(fs::read(&path)?, before);
        revised.confirm_current_location()?;
        assert!(!revised.needs_recovery());
        assert!(!revised.playing);
        assert_eq!(revised.current, 20);
        revised.save(&path)?;
        assert_ne!(fs::read(&path)?, before);
        fs::remove_file(path)?;
        Ok(())
    }
}
