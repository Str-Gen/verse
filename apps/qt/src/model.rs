use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[cfg(test)]
use std::fs;

use reader_core::{Book, book_store::DiskBook};
use reader_core::{location, session};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

struct Recovery {
    position: usize,
    description: String,
}
#[derive(Default, Serialize, Deserialize)]
struct SavedBooks {
    books: BTreeMap<String, Value>,
}

struct NotePlayback {
    note: usize,
    at: usize,
    actions: Vec<session::Playback>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Feedback {
    chapter: String,
    pace: String,
    understanding: String,
    speed_before: u32,
    speed_after: u32,
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
    store: Option<DiskBook>,
    directory: std::path::PathBuf,
    pub window_start: usize,
    pub window_generation: u64,
    positions: Vec<(usize, usize)>,
    chapter_positions: Vec<usize>,
    preserve_saved: bool,
    inspected_stop: Option<usize>,
    recovery: Option<Recovery>,
    pub follow_notes: bool,
    pub feedback_enabled: bool,
    note_playback: Option<NotePlayback>,
    completed_chapter: Option<String>,
    history: Vec<Feedback>,
}

impl Model {
    pub fn book(&self) -> &Book {
        &self.book
    }
    pub fn exposed_index(&self) -> Option<usize> {
        (self.note_playback.is_none()
            && session::kind(&self.book.pieces[self.current]) == session::Kind::Word)
            .then_some(self.current)
    }
    pub fn open(path: impl AsRef<Path>, import_directory: &Path) -> Result<Self, String> {
        let path = path.as_ref();
        if !path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("epub"))
        {
            return Self::load(path);
        }
        let identities = location::LocalStore::open(import_directory.join("identities"))
            .map_err(|error| format!("Cannot open the book library: {error}"))?;
        let book_id = identities
            .identity_for(path)
            .map_err(|error| format!("Cannot open EPUB: {error}"))?;
        let output = import_directory.join(format!("{book_id}.json"));
        reader_core::export_epub(
            path,
            &book_id,
            &reader_core::ImportOptions::default(),
            &output,
        )
        .map_err(|error| format!("Cannot import EPUB: {error}"))?;
        Self::load(output)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let store = DiskBook::open(path).map_err(|e| e.to_string())?;
        let mut window = store.load_window(0).map_err(|e| e.to_string())?;
        window.book.resources = store.image_resources(0).map_err(|e| e.to_string())?;
        let mut model =
            Self::from_book(window.book, path.parent().unwrap_or_else(|| Path::new(".")))?;
        model.chapter_positions = store.chapter_starts().to_vec();
        model.window_start = window.start;
        model.store = Some(store);
        Ok(model)
    }

    #[cfg(test)]
    fn from_json(json: &str, directory: &Path) -> Result<Self, String> {
        let book: Book = session::book_from_value(
            serde_json::from_str(json).map_err(|e| format!("Invalid book JSON: {e}"))?,
        )
        .map_err(|e| format!("Invalid book JSON: {e}"))?;
        Self::from_book(book, directory)
    }

    fn from_book(book: Book, directory: &Path) -> Result<Self, String> {
        if book.schema_version != 1 {
            return Err("Only book JSON schema version 1 is supported".into());
        }
        if book.pieces.is_empty() {
            return Err("Book has no playback pieces".into());
        }
        validate(&book)?;
        let (html, plain_text, positions) = render(&book, directory)?;
        let piece_indices: BTreeMap<_, _> = book
            .pieces
            .iter()
            .enumerate()
            .map(|(i, p)| (p.id.as_str(), i))
            .collect();
        let chapter_positions = book
            .chapters
            .iter()
            .filter_map(|c| piece_indices.get(c.start_piece.as_str()).copied())
            .collect();
        Ok(Self {
            chapter_positions,
            title: book.title.clone(),
            html,
            plain_text,
            current: 0,
            playing: false,
            speed: 350,
            status: "Stopped".into(),
            book,
            store: None,
            directory: directory.to_owned(),
            window_start: 0,
            window_generation: 0,
            positions,
            preserve_saved: false,
            inspected_stop: None,
            recovery: None,
            follow_notes: false,
            feedback_enabled: true,
            note_playback: None,
            completed_chapter: None,
            history: Vec::new(),
        })
    }

    pub fn images_json(&self) -> String {
        let mut sources: Vec<String> = self
            .book
            .resources
            .iter()
            .filter_map(|resource| {
                if resource.data_url.is_empty() {
                    local_image_uri(&resource.id, &self.directory).ok()
                } else {
                    Some(resource.data_url.clone())
                }
            })
            .collect();
        let markers: Vec<_> = self
            .book
            .pieces
            .iter()
            .enumerate()
            .filter(|(_, piece)| piece.kind == "image")
            .collect();
        let before = markers
            .iter()
            .filter(|(at, _)| *at < self.current)
            .rev()
            .take(2);
        let after = markers.iter().filter(|(at, _)| *at >= self.current).take(3);
        for (_, piece) in before.chain(after) {
            if let Some(resource) = self
                .book
                .documents
                .iter()
                .flat_map(|document| &document.blocks)
                .find(|block| block.id == piece.block_id)
                .and_then(|block| block.resource.as_deref())
                && !self.book.resources.iter().any(|entry| entry.id == resource)
                && let Ok(uri) = local_image_uri(resource, &self.directory)
                && !sources.contains(&uri)
                && sources.len() < 5
            {
                sources.push(uri);
            }
        }
        json!(sources).to_string()
    }

    pub fn progress_percent(&self) -> f64 {
        let total = self
            .store
            .as_ref()
            .map_or(self.book.pieces.len(), DiskBook::total_pieces);
        if total <= 1 {
            return 0.0;
        }
        self.global_position() as f64 / (total - 1) as f64 * 100.0
    }

    pub fn global_position(&self) -> usize {
        self.window_start + self.current
    }

    pub fn seek(&mut self, global: usize) -> Result<(), String> {
        self.seek_window(global, false)
    }

    fn seek_window(&mut self, global: usize, force: bool) -> Result<(), String> {
        let Some(store) = &self.store else {
            self.current = global.min(self.book.pieces.len() - 1);
            return Ok(());
        };
        if global >= store.total_pieces() {
            return Err("Passage is outside this book".into());
        }
        if force
            || !(self.window_start..self.window_start + self.book.pieces.len()).contains(&global)
        {
            let mut window = store.load_window(global).map_err(|e| e.to_string())?;
            window.book.resources = store.image_resources(global).map_err(|e| e.to_string())?;
            let (html, plain, positions) = render(&window.book, &self.directory)?;
            self.book = window.book;
            self.window_start = window.start;
            self.html = html;
            self.plain_text = plain;
            self.positions = positions;
            self.window_generation += 1;
            self.inspected_stop = None;
        }
        self.current = global - self.window_start;
        Ok(())
    }

    pub fn turn_window(&mut self, direction: i32) {
        self.return_to_text();
        let target = if direction < 0 {
            self.window_start.checked_sub(1)
        } else {
            Some(self.window_start + self.book.pieces.len()).filter(|&at| {
                self.store
                    .as_ref()
                    .is_some_and(|store| at < store.total_pieces())
            })
        };
        if let Some(target) = target
            && let Err(error) = self.seek(target)
        {
            self.status = error;
        }
    }

    fn metadata(&self) -> &Book {
        self.store.as_ref().map_or(&self.book, DiskBook::metadata)
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
        self.note_playback.as_ref().map_or_else(
            || self.book.pieces[self.current].text.clone(),
            |playback| {
                self.book.notes[playback.note].pieces[playback.at]
                    .text
                    .clone()
            },
        )
    }
    pub fn current_utf16(&self) -> usize {
        self.positions[self.current].0
    }
    pub fn current_utf16_end(&self) -> usize {
        self.positions[self.current].1
    }

    pub fn chapters_json(&self) -> String {
        json!(
            self.metadata()
                .chapters
                .iter()
                .map(|c| &c.title)
                .collect::<Vec<_>>()
        )
        .to_string()
    }

    pub fn chapter_position(&self, index: usize) -> Option<usize> {
        self.chapter_positions
            .get(index)
            .and_then(|&at| at.checked_sub(self.window_start))
            .and_then(|at| self.positions.get(at).map(|p| p.0))
    }

    pub fn chapter_index(&self) -> i32 {
        self.chapter_positions
            .iter()
            .rposition(|&at| at <= self.global_position())
            .and_then(|i| i32::try_from(i).ok())
            .unwrap_or(-1)
    }

    pub fn jump_chapter(&mut self, index: usize) {
        let Some(&at) = self.chapter_positions.get(index) else {
            return;
        };
        self.return_to_text();
        self.completed_chapter = None;
        if let Err(error) = self.seek(at) {
            self.status = error;
            return;
        }
        self.inspected_stop = None;
        self.status = "Selected chapter".into();
    }

    pub fn return_to_text(&mut self) {
        self.pause();
        self.note_playback = None;
    }

    pub fn feedback_chapter(&self) -> String {
        self.completed_chapter.clone().unwrap_or_default()
    }

    pub fn history_text(&self) -> String {
        if self.history.is_empty() {
            return "No chapter feedback saved.".into();
        }
        self.history
            .iter()
            .rev()
            .map(|entry| {
                format!(
                    "{}\nPace: {}; followed: {}\n{} → {} WPM",
                    entry.chapter,
                    entry.pace,
                    entry.understanding,
                    entry.speed_before,
                    entry.speed_after
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    pub fn finish_feedback(&mut self, pace: &str, understanding: &str, accept_speed: bool) {
        let Some(chapter) = self.completed_chapter.take() else {
            return;
        };
        if !matches!(pace, "Comfortable" | "Too slow" | "Too fast") {
            return;
        }
        let before = self.speed;
        if accept_speed {
            self.speed = session::speed(match pace {
                "Too slow" => self.speed.saturating_add(25),
                "Too fast" => self.speed.saturating_sub(25),
                _ => self.speed,
            });
        }
        self.history.push(Feedback {
            chapter,
            pace: pace.into(),
            understanding: understanding.into(),
            speed_before: before,
            speed_after: self.speed,
        });
    }

    fn complete_chapter(&mut self) {
        if self.feedback_enabled {
            self.completed_chapter = usize::try_from(self.chapter_index())
                .ok()
                .and_then(|i| self.metadata().chapters.get(i))
                .map(|c| c.title.clone());
        }
    }

    fn enter_note(&mut self) -> bool {
        if !self.follow_notes
            || session::kind(&self.book.pieces[self.current]) != session::Kind::Note
        {
            return false;
        }
        let id = self.book.pieces[self.current].note_id.as_ref();
        let found = self.book.notes.iter().position(|n| Some(&n.id) == id);
        if let Some(note) = found {
            let pieces = &self.book.notes[note].pieces;
            if let Some(at) = pieces.iter().position(|p| !session::skip(p, false)) {
                self.note_playback = Some(NotePlayback {
                    note,
                    at,
                    actions: session::plan(pieces, &[], false),
                });
                self.update_note_status();
                return true;
            }
        }
        self.playing = false;
        self.inspected_stop = Some(self.current);
        self.status = "Note unavailable. Press play to continue.".into();
        true
    }

    fn update_note_status(&mut self) {
        if let Some(note) = &self.note_playback {
            self.playing = note.actions[note.at].kind == session::Kind::Word;
            self.status = if self.playing {
                "Reading note. Return to text goes to its reference."
            } else {
                "Note contains an image or unsupported content. Press play to continue."
            }
            .into();
        }
    }

    pub fn select_utf16(&mut self, position: usize) {
        self.note_playback = None;
        self.completed_chapter = None;
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
        if let Err(error) = self.refresh_images() {
            self.status = error;
        }
    }

    fn refresh_images(&mut self) -> Result<(), String> {
        let Some(store) = &self.store else {
            return Ok(());
        };
        let resources = store
            .image_resources(self.global_position())
            .map_err(|e| e.to_string())?;
        if (!resources.is_empty() || !self.book.pieces.iter().any(|piece| piece.kind == "image"))
            && resources
                .iter()
                .map(|r| &r.id)
                .eq(self.book.resources.iter().map(|r| &r.id))
        {
            return Ok(());
        }
        self.book.resources = resources;
        let (html, plain, positions) = render(&self.book, &self.directory)?;
        self.html = html;
        self.plain_text = plain;
        self.positions = positions;
        self.window_generation += 1;
        Ok(())
    }

    pub fn start(&mut self) {
        self.completed_chapter = None;
        if let Some(note) = &self.note_playback {
            self.playing = true;
            if note.actions[note.at].kind != session::Kind::Word {
                self.step();
            }
            return;
        }
        if self.inspected_stop == Some(self.current)
            && session::kind(&self.book.pieces[self.current]) == session::Kind::Note
        {
            self.playing = true;
            self.step();
            return;
        }
        if matches!(
            session::kind(&self.book.pieces[self.current]),
            session::Kind::Image | session::Kind::Unsupported
        ) {
            if self.inspected_stop != Some(self.current) {
                self.stop_for_inspection();
                return;
            }
            self.playing = true;
            self.step();
            if !self.playing {
                return;
            }
        }
        if !self.skip_manual() {
            return;
        }
        if self.enter_note() {
            return;
        }
        if matches!(
            session::kind(&self.book.pieces[self.current]),
            session::Kind::Image | session::Kind::Unsupported
        ) {
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
        if let Some(note) = &mut self.note_playback {
            if let Some(next) = note.actions[note.at].next {
                note.at = next;
                self.update_note_status();
                return;
            }
            self.note_playback = None;
            self.status = "Playing".into();
        }
        if self.store.is_some() {
            if let Err(error) = self.step_disk() {
                self.playing = false;
                self.status = error;
            }
            return;
        }
        let Some(action) = session::playback(&self.book, self.current, self.follow_notes) else {
            return;
        };
        let Some(next) = action.next else {
            self.complete_chapter();
            self.playing = false;
            self.status = "End of book".into();
            return;
        };
        if action.chapter_boundary {
            self.complete_chapter();
        }
        self.current = next;
        if session::kind(&self.book.pieces[self.current]) == session::Kind::Image
            && let Err(error) = self.seek_window(self.global_position(), true)
        {
            self.playing = false;
            self.status = error;
            return;
        }
        if action.chapter_boundary {
            self.playing = false;
            self.status = "Chapter boundary. Press play to continue.".into();
            return;
        }
        if self.enter_note() {
            return;
        }
        if !matches!(
            session::kind(&self.book.pieces[self.current]),
            session::Kind::Word
        ) {
            self.stop_for_inspection();
        }
    }

    fn step_disk(&mut self) -> Result<(), String> {
        let global = self.global_position();
        let leaving_image = session::kind(&self.book.pieces[self.current]) == session::Kind::Image;
        let store = self.store.as_ref().ok_or("No book open")?;
        let mut next = global + 1;
        while next < store.total_pieces()
            && session::skip(
                &store.piece(next).map_err(|e| e.to_string())?,
                self.follow_notes,
            )
        {
            next += 1;
        }
        if next == store.total_pieces() {
            self.complete_chapter();
            self.playing = false;
            self.status = "End of book".into();
            return Ok(());
        }
        let boundary = self
            .chapter_positions
            .iter()
            .any(|&at| at > global && at <= next);
        if boundary {
            self.complete_chapter();
        }
        self.seek(next)?;
        if leaving_image || session::kind(&self.book.pieces[self.current]) == session::Kind::Image {
            self.refresh_images()?;
        }
        if boundary {
            self.playing = false;
            self.status = "Chapter boundary. Press play to continue.".into();
        } else if !self.enter_note()
            && session::kind(&self.book.pieces[self.current]) != session::Kind::Word
        {
            self.stop_for_inspection();
        }
        Ok(())
    }

    pub fn delay_ms(&self) -> u32 {
        if let Some(note) = &self.note_playback {
            let action = &note.actions[note.at];
            return (60_000.0 / f64::from(session::speed(self.speed)) * action.multiplier).round()
                as u32
                + action.extra_ms;
        }
        session::delay_ms(&self.book, self.current, self.speed)
    }

    fn skip_manual(&mut self) -> bool {
        if let Some(store) = &self.store {
            let mut at = self.global_position();
            while at < store.total_pieces() {
                match store.piece(at) {
                    Ok(piece) if session::skip(&piece, self.follow_notes) => at += 1,
                    Ok(_) => {
                        if let Err(error) = self.seek(at) {
                            self.status = error;
                            self.playing = false;
                            return false;
                        }
                        return true;
                    }
                    Err(error) => {
                        self.status = error.to_string();
                        self.playing = false;
                        return false;
                    }
                }
            }
            self.playing = false;
            self.status = "End of book".into();
            return false;
        }
        if let Some(at) = session::start(&self.book, self.current, self.follow_notes) {
            self.current = at;
            true
        } else {
            self.playing = false;
            self.status = "End of book".into();
            false
        }
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
        if let Some(store) = &self.store {
            let preferences = json!({"speed":self.speed,"follow_notes":self.follow_notes,
                "feedback_enabled":self.feedback_enabled,"qt_history":self.history});
            let record = store
                .capture(self.global_position(), &preferences)
                .map_err(|e| e.to_string())?;
            let chapter_start = self
                .chapter_positions
                .iter()
                .copied()
                .rfind(|&at| at <= self.global_position())
                .unwrap_or(0);
            let chapter = store
                .capture(chapter_start, &preferences)
                .map_err(|e| e.to_string())?;
            return reader_core::persistence::save_record_for_book(path, store, &record, &chapter)
                .map_err(|e| e.to_string());
        }
        let mut saved = read_saved(path)?;
        let mut preferences = saved
            .books
            .get(&self.book.book_id)
            .cloned()
            .unwrap_or(json!({}));
        preferences["speed"] = json!(self.speed);
        preferences["follow_notes"] = json!(self.follow_notes);
        preferences["feedback_enabled"] = json!(self.feedback_enabled);
        preferences["qt_history"] = json!(self.history);
        saved.books.insert(
            self.book.book_id.clone(),
            session::capture(&self.book, self.current, &preferences).map_err(|e| e.to_string())?,
        );
        location::write_json(path, &saved).map_err(|e| e.to_string())
    }

    pub fn recovery_description(&self) -> Option<String> {
        self.recovery.as_ref().map(|recovery| {
            let position = recovery
                .position
                .saturating_sub(self.window_start)
                .min(self.book.pieces.len() - 1);
            let start = position.saturating_sub(3);
            let end = (position + 4).min(self.book.pieces.len());
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
        self.seek(recovery.position)?;
        self.playing = false;
        self.inspected_stop = None;
        self.preserve_saved = false;
        self.status = "Recovered location confirmed, stopped".into();
        Ok(())
    }

    pub fn restore(&mut self, path: impl AsRef<Path>) -> Result<(), String> {
        self.playing = false;
        self.recovery = None;
        if let Some(store) = &self.store {
            let Some(state) = reader_core::persistence::load_record_for_book(path.as_ref(), store)
                .map_err(|e| e.to_string())?
            else {
                return Ok(());
            };
            let resume = store.restore(Some(&state)).map_err(|e| e.to_string())?;
            self.follow_notes = state["follow_notes"].as_bool().unwrap_or(false);
            self.feedback_enabled = state["feedback_enabled"].as_bool().unwrap_or(true);
            let history = state
                .get("qt_history")
                .map(|history| serde_json::from_value(history.clone()))
                .transpose();
            let invalid_history = history.is_err();
            self.history = history.unwrap_or_default().unwrap_or_default();
            self.speed = session::speed(
                state["speed"]
                    .as_u64()
                    .and_then(|speed| u32::try_from(speed).ok())
                    .unwrap_or(350),
            );
            if matches!(resume.kind.as_str(), "exact" | "fresh") {
                self.seek(resume.at)?;
                self.status = if invalid_history {
                    "Restored passage, stopped. Invalid feedback history was discarded."
                } else {
                    "Restored passage, stopped"
                }
                .into();
                self.preserve_saved = false;
                return Ok(());
            }
            self.preserve_saved = true;
            self.recovery =
                matches!(resume.kind.as_str(), "passage" | "chapter").then_some(Recovery {
                    position: resume.at,
                    description: if resume.kind == "chapter" {
                        "Confirm to resume at the saved chapter's start, stopped."
                    } else {
                        "A matching passage was found. Confirm to resume there."
                    }
                    .into(),
                });
            self.status = "Saved location requires confirmation; original state preserved".into();
            return Err(self.status.clone());
        }
        let saved = read_saved(path.as_ref())?;
        let Some(state) = saved.books.get(&self.book.book_id) else {
            return Ok(());
        };
        self.preserve_saved = true;
        self.follow_notes = state["follow_notes"].as_bool().unwrap_or(false);
        self.feedback_enabled = state["feedback_enabled"].as_bool().unwrap_or(true);
        if let Some(history) = state.get("qt_history") {
            self.history = serde_json::from_value(history.clone())
                .map_err(|e| format!("Invalid feedback history: {e}"))?;
        }
        self.speed = session::speed(
            state
                .get("speed")
                .and_then(Value::as_u64)
                .and_then(|v| u32::try_from(v).ok())
                .unwrap_or(350),
        );
        let resume = session::restore(&self.book, Some(state)).map_err(|e| e.to_string())?;
        if resume.kind == "exact" {
            self.current = resume.at;
            self.preserve_saved = false;
            self.status = "Restored passage, stopped".into();
            return Ok(());
        }
        self.preserve_saved = true;
        self.recovery = match resume.kind.as_str() {
            "passage" => Some(Recovery { position: resume.at, description: "A unique matching passage was found in this edition. Confirm to resume there, stopped.".into() }),
            "chapter" => Some(Recovery { position: resume.at, description: "Confirm to resume at the saved chapter's start, stopped.".into() }),
            _ => None,
        };
        self.status = "Saved location requires confirmation; original state preserved".into();
        Err(self.status.clone())
    }
}

fn read_saved(path: &Path) -> Result<SavedBooks, String> {
    match location::read_json(path) {
        Ok(saved) => Ok(saved),
        Err(location::LocationError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {
            Ok(SavedBooks::default())
        }
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
                let uri = if let Some(embedded) = book
                    .resources
                    .iter()
                    .find(|r| &r.id == resource && !r.data_url.is_empty())
                {
                    if ![
                        "data:image/png;base64,",
                        "data:image/gif;base64,",
                        "data:image/jpeg;base64,",
                        "data:image/webp;base64,",
                    ]
                    .iter()
                    .any(|prefix| embedded.data_url.starts_with(prefix))
                    {
                        return Err("Unsupported embedded image format".into());
                    }
                    format!(
                        "reader-image:{:x}",
                        Sha256::digest(embedded.data_url.as_bytes())
                    )
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

    #[test]
    fn epub_open_reimports_with_stable_identity_and_restores_progress()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("qt-epub-import-{}", std::process::id()));
        fs::create_dir_all(&root)?;
        let fixtures =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/reader-core/fixtures");
        let epub = root.join("book with spaces.EPUB");
        fs::copy(fixtures.join("complex.epub"), &epub)?;
        let imports = root.join("imports");
        let state = root.join("resume.json");
        let mut model = Model::open(&epub, &imports)?;
        assert!(!model.plain_text.is_empty());
        model.jump_chapter(1);
        model.speed = 420;
        model.save(&state)?;
        let book_id = model.book_id().to_owned();
        let revision = model.book().revision.clone();
        let mut reopened = Model::open(&epub, &imports)?;
        assert_eq!(reopened.book_id(), book_id);
        reopened.restore(&state)?;
        assert_eq!(reopened.global_position(), model.global_position());
        assert_eq!(reopened.speed, 420);
        assert!(!reopened.playing);
        fs::copy(fixtures.join("revised.epub"), &epub)?;
        let revised = Model::open(&epub, &imports)?;
        assert_eq!(revised.book_id(), book_id);
        assert_ne!(revised.book().revision, revision);
        fs::write(&epub, "invalid EPUB")?;
        assert!(Model::open(&epub, &imports).is_err());
        assert!(!model.plain_text.is_empty());
        assert!(Model::open(root.join("missing.epub"), &imports).is_err());
        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn disk_windows_traverse_and_restore_without_retaining_the_book()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("qt-disk-window-{}", std::process::id()));
        fs::create_dir_all(&root)?;
        let path = root.join("book.json");
        fs::write(&path, include_str!("../fixtures/demo.json"))?;
        let mut model = Model::load(&path)?;
        assert!(model.book().pieces.len() <= 512);
        assert_eq!(model.progress_percent(), 0.0);
        let mut visited = vec![model.current_text()];
        model.feedback_enabled = false;
        for _ in 0..800 {
            model.start();
            model.step();
            if model.status == "End of book" {
                break;
            }
            visited.push(model.current_text());
            assert!(model.book().pieces.len() <= 512);
            assert!(model.html.len() < 100_000);
        }
        assert!(visited.len() > 600);
        assert!(model.global_position() > 600);
        assert_eq!(model.status, "End of book");
        assert!(model.progress_percent() > 99.0);
        let global = model.global_position();
        model.save(root.join("resume.json"))?;
        model.save(root.join("resume.json"))?;
        let saved_path =
            reader_core::persistence::record_path(&root.join("resume.json"), model.book_id());
        let mut saved: Value = location::read_json(&saved_path)?;
        saved["qt_history"] = json!("corrupt optional preference");
        location::write_json(&saved_path, &saved)?;
        let mut resumed = Model::load(&path)?;
        resumed.restore(root.join("resume.json"))?;
        assert_eq!(resumed.global_position(), global);
        assert!(resumed.status.contains("Invalid feedback history"));
        saved["location"]["sources"][0]["node"] = json!("missing-source");
        location::write_json(&saved_path, &saved)?;
        let mut recovered = Model::load(&path)?;
        recovered.restore(root.join("resume.json"))?;
        assert_eq!(recovered.global_position(), global);
        assert!(!recovered.needs_recovery());
        resumed.jump_chapter(0);
        assert_eq!(resumed.global_position(), 0);
        resumed.jump_chapter(1);
        assert_eq!(resumed.global_position(), 472);
        resumed.turn_window(1);
        resumed.turn_window(-1);
        assert!(resumed.global_position() < 472);
        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn chapter_navigation_feedback_and_preferences_round_trip()
    -> Result<(), Box<dyn std::error::Error>> {
        let path = std::env::temp_dir().join(format!("qt-parity-{}.json", std::process::id()));
        let mut model = demo()?;
        model.follow_notes = true;
        model.jump_chapter(1);
        assert_eq!(model.chapter_index(), 1);
        assert!(!model.playing);
        let next = model.current;
        model.current = next - 1;
        model.playing = true;
        model.step();
        assert!(!model.playing);
        assert!(!model.feedback_chapter().is_empty());
        model.finish_feedback("", "", false);
        assert!(model.history.is_empty());
        model.current = next - 1;
        model.playing = true;
        model.step();
        model.finish_feedback("Too fast", "Mostly", false);
        assert_eq!(model.speed, 350);
        model.current = next - 1;
        model.playing = true;
        model.step();
        model.finish_feedback("Too fast", "Struggled", true);
        assert_eq!(model.speed, 325);
        model.save(&path)?;
        let mut restored = demo()?;
        restored.restore(&path)?;
        assert!(restored.follow_notes);
        assert_eq!(restored.history.len(), 2);
        assert!(restored.history_text().contains("350 → 325 WPM"));
        assert_eq!(restored.current, next);
        restored.clear_history();
        restored.save(&path)?;
        model.restore(&path)?;
        assert!(model.history.is_empty());
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn missing_note_can_continue_and_disabled_feedback_stays_quiet()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut model = demo()?;
        model.follow_notes = true;
        model.book.pieces[3].kind = "note".into();
        model.book.pieces[3].note_id = Some("missing".into());
        model.current = 3;
        model.start();
        assert!(!model.playing);
        assert!(model.status.contains("unavailable"));
        model.start();
        assert_eq!(model.current, 4);
        assert!(model.playing);
        model.jump_chapter(1);
        model.current -= 1;
        model.feedback_enabled = false;
        model.playing = true;
        model.step();
        assert!(!model.playing);
        assert!(model.feedback_chapter().is_empty());
        Ok(())
    }

    #[test]
    fn optional_notes_return_and_save_the_main_text_anchor()
    -> Result<(), Box<dyn std::error::Error>> {
        let path = std::env::temp_dir().join(format!("qt-note-parity-{}.json", std::process::id()));
        let mut model = demo()?;
        model.book.pieces[3].kind = "note".into();
        model.book.pieces[3].note_id = Some("n".into());
        let mut pieces = model.book.pieces[4..6].to_vec();
        pieces[0].text = "Note,".into();
        pieces[1].text = "done.".into();
        model.book.notes.push(reader_core::Note {
            id: "n".into(),
            blocks: vec![],
            pieces,
        });
        model.current = 3;
        model.start();
        assert_eq!(model.current, 4);
        model.return_to_text();
        model.follow_notes = true;
        model.current = 3;
        model.speed = 600;
        model.start();
        assert_eq!(model.current_text(), "Note,");
        assert_eq!(model.delay_ms(), 125);
        model.pause();
        model.start();
        assert_eq!(model.current_text(), "Note,");
        model.step();
        assert_eq!(model.current_text(), "done.");
        model.save(&path)?;
        let saved = read_saved(&path)?;
        assert_eq!(
            saved.books[model.book_id()]["sources"],
            json!(model.book.pieces[3].sources)
        );
        model.step();
        assert_eq!(model.current, 4);
        assert!(model.note_playback.is_none());
        assert!(model.playing);
        model.current = 3;
        model.start();
        model.return_to_text();
        assert_eq!(model.current, 3);
        assert!(model.note_playback.is_none());
        assert!(!model.playing);
        fs::remove_file(path)?;
        Ok(())
    }

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
        assert_eq!(
            session::kind(&model.book.pieces[model.current]),
            session::Kind::Image
        );
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
    fn changed_revision_offers_chapter_start_without_passage_matching()
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
                .is_some_and(|description| description.contains("chapter's start"))
        );
        assert!(revised.save(&path).is_err());
        assert_eq!(fs::read(&path)?, before);
        revised.confirm_recovery()?;
        assert_eq!(revised.current, 0);
        assert!(!revised.playing);
        revised.save(&path)?;
        let saved = read_saved(&path)?;
        assert_eq!(
            saved.books.get(revised.book_id()).ok_or("saved book")?["revision"],
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
        record.remove("location");
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

#[cfg(test)]
#[path = "model_adversarial.rs"]
mod adversarial;
