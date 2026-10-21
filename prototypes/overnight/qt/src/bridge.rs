use crate::model::Model;
use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use reader_core::{
    experience::{Comfort, Exposure},
    vocabulary::Vocabulary,
};
use serde_json::{Value, json};
use std::time::Instant;
use std::{path::PathBuf, pin::Pin};

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, title)]
        #[qproperty(QString, html)]
        #[qproperty(QString, plain_text)]
        #[qproperty(QString, word)]
        #[qproperty(QString, status)]
        #[qproperty(QString, recovery)]
        #[qproperty(bool, recovery_needed)]
        #[qproperty(i32, position)]
        #[qproperty(i32, end_position)]
        #[qproperty(i32, speed)]
        #[qproperty(i32, delay_ms)]
        #[qproperty(bool, playing)]
        #[qproperty(bool, preparing)]
        #[qproperty(QString, chapters)]
        #[qproperty(i32, chapter_index)]
        #[qproperty(bool, follow_notes)]
        #[qproperty(bool, feedback_enabled)]
        #[qproperty(QString, feedback_chapter)]
        #[qproperty(QString, history)]
        type Reader = super::ReaderRust;

        #[qinvokable]
        fn vocabulary(self: Pin<&mut Reader>, request: &QString) -> QString;
        #[qinvokable]
        fn chapter_position(self: &Reader, index: i32) -> i32;
        #[qinvokable]
        fn jump_chapter(self: Pin<&mut Reader>, index: i32);
        #[qinvokable]
        fn return_to_text(self: Pin<&mut Reader>);
        #[qinvokable]
        fn set_options(self: Pin<&mut Reader>, notes: bool, feedback: bool);
        #[qinvokable]
        fn finish_feedback(
            self: Pin<&mut Reader>,
            pace: &QString,
            understanding: &QString,
            accept_speed: bool,
        );
        #[qinvokable]
        fn clear_history(self: Pin<&mut Reader>);
        #[qinvokable]
        fn note_text(self: &Reader, id: &QString) -> QString;
        #[qinvokable]
        fn initialize(self: Pin<&mut Reader>);
        #[qinvokable]
        fn open_book(self: Pin<&mut Reader>, path: &QString);
        #[qinvokable]
        fn select(self: Pin<&mut Reader>, position: i32);
        #[qinvokable]
        fn play(self: Pin<&mut Reader>);
        #[qinvokable]
        fn pause(self: Pin<&mut Reader>);
        #[qinvokable]
        fn tick(self: Pin<&mut Reader>);
        #[qinvokable]
        fn change_speed(self: Pin<&mut Reader>, value: i32);
        #[qinvokable]
        fn persist(self: Pin<&mut Reader>);
        #[qinvokable]
        fn confirm_recovery(self: Pin<&mut Reader>);
        #[qinvokable]
        fn confirm_selection(self: Pin<&mut Reader>);
    }
}

#[derive(Default)]
pub struct ReaderRust {
    title: QString,
    html: QString,
    plain_text: QString,
    word: QString,
    status: QString,
    recovery: QString,
    recovery_needed: bool,
    position: i32,
    end_position: i32,
    speed: i32,
    delay_ms: i32,
    playing: bool,
    preparing: bool,
    clock: Option<Instant>,
    comfort: Comfort,
    exposures: Vec<Exposure>,
    exposure: Option<(usize, u64)>,
    vocabulary: Option<Vocabulary>,
    chapters: QString,
    chapter_index: i32,
    follow_notes: bool,
    feedback_enabled: bool,
    feedback_chapter: QString,
    history: QString,
    model: Option<Model>,
    book_lock: Option<reader_core::persistence::BookLock>,
    state_path: PathBuf,
}

fn argument(name: &str) -> Option<String> {
    let args: Vec<_> = std::env::args().collect();
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

impl ffi::Reader {
    fn now(&self) -> u64 {
        self.rust().clock.map_or(0, |clock| {
            u64::try_from(clock.elapsed().as_millis()).unwrap_or(u64::MAX)
        })
    }
    fn finish_exposure(mut self: Pin<&mut Self>) {
        let now = self.now();
        let mut rust = self.as_mut().rust_mut();
        if let Some((piece, start)) = rust.exposure.take()
            && now > start
        {
            rust.exposures.push(Exposure {
                piece,
                start,
                end: now,
            });
            if rust.exposures.len() > 2000 {
                rust.exposures.remove(0);
            }
        }
    }
    fn begin_exposure(mut self: Pin<&mut Self>) {
        let now = self.now();
        let mut rust = self.as_mut().rust_mut();
        rust.exposure = rust
            .model
            .as_ref()
            .filter(|m| m.playing)
            .and_then(|m| m.exposed_index())
            .map(|i| (i, now));
    }
    pub fn vocabulary(mut self: Pin<&mut Self>, request: &QString) -> QString {
        let result = (|| -> Result<Value, String> {
            let mut request: Value =
                serde_json::from_str(&request.to_string()).map_err(|e| e.to_string())?;
            if request["action"] == "capture" {
                self.as_mut().finish_exposure();
                request["exposures"] = json!(self.rust().exposures);
                request["now"] = json!(self.now());
                if !self.rust().preparing {
                    self.as_mut().begin_exposure();
                }
            }
            if self.rust().vocabulary.is_none() {
                let mut rust = self.as_mut().rust_mut();
                let model = rust.model.as_ref().ok_or("No book open")?;
                let root = rust.state_path.parent().ok_or("No state directory")?;
                rust.vocabulary = Some(Vocabulary::open(root, model.book())?);
            }
            self.as_mut()
                .rust_mut()
                .vocabulary
                .as_mut()
                .ok_or("Vocabulary unavailable")?
                .request(&request)
        })();
        QString::from(
            match result {
                Ok(value) => json!({"result":value}),
                Err(error) => json!({"error":error}),
            }
            .to_string(),
        )
    }

    pub fn note_text(&self, id: &QString) -> QString {
        QString::from(self.rust().model.as_ref().map_or_else(
            || "No book open".into(),
            |model| model.note_text(&id.to_string()),
        ))
    }
    pub fn initialize(mut self: Pin<&mut Self>) {
        let directory = argument("--state-dir")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("READER_STATE_DIR").map(PathBuf::from))
            .unwrap_or_else(|| {
                std::env::var_os("XDG_STATE_HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| {
                        PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                            .join(".local/state")
                    })
                    .join("overnight-qt-reader")
            });
        self.as_mut().rust_mut().state_path = directory.join("resume.json");
        let mut warning = None;
        let remembered = match std::fs::read_to_string(directory.join("last-book")) {
            Ok(path) => Some(path),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                warning = Some(format!("Cannot read the last-book path: {error}"));
                None
            }
        };
        let path = argument("--book-json")
            .or(remembered)
            .unwrap_or_else(|| concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/demo.json").into());
        self.as_mut().open_book(&QString::from(path));
        if let Some(warning) = warning {
            self.set_status(QString::from(warning));
        }
    }

    pub fn open_book(mut self: Pin<&mut Self>, path: &QString) {
        match Model::load(path.to_string()) {
            Ok(mut model) => {
                let new_lock = if self
                    .rust()
                    .book_lock
                    .as_ref()
                    .is_some_and(|lock| lock.book_id() == model.book_id())
                {
                    None
                } else {
                    match reader_core::persistence::BookLock::acquire(model.book_id()) {
                        Ok(lock) => Some(lock),
                        Err(error) => {
                            self.set_status(QString::from(error));
                            return;
                        }
                    }
                };
                self.as_mut().finish_exposure();
                let mut rust = self.as_mut().rust_mut();
                rust.clock = Some(Instant::now());
                rust.comfort = Comfort::default();
                rust.exposures.clear();
                rust.vocabulary = None;
                self.as_mut().set_preparing(false);
                let previous_error = self
                    .rust()
                    .model
                    .as_ref()
                    .and_then(|current| current.save(&self.rust().state_path).err());
                if let Some(lock) = new_lock {
                    self.as_mut().rust_mut().book_lock = Some(lock);
                }
                if let Err(error) = model.restore(&self.rust().state_path) {
                    model.status = error;
                }
                if let Some(error) = previous_error {
                    model.status = format!("{error}. New book opened; previous state retained.");
                }
                if let Some(directory) = self.rust().state_path.parent() {
                    let remembered =
                        std::fs::canonicalize(path.to_string()).and_then(|book_path| {
                            std::fs::create_dir_all(directory)?;
                            std::fs::write(
                                directory.join("last-book"),
                                book_path.to_string_lossy().as_bytes(),
                            )
                        });
                    if let Err(error) = remembered {
                        model.status =
                            format!("Book opened, but cannot remember its path: {error}");
                    }
                }
                let vocabulary = self
                    .rust()
                    .state_path
                    .parent()
                    .ok_or_else(|| "No state directory".to_owned())
                    .and_then(|root| Vocabulary::open(root, model.book()));
                match vocabulary {
                    Ok(worker) => self.as_mut().rust_mut().vocabulary = Some(worker),
                    Err(error) => {
                        model.status = format!("Book opened; vocabulary unavailable: {error}")
                    }
                }
                self.as_mut()
                    .set_chapters(QString::from(model.chapters_json()));
                self.as_mut()
                    .set_history(QString::from(model.history_text()));
                self.as_mut().set_title(QString::from(&model.title));
                self.as_mut().set_html(QString::from(&model.html));
                self.as_mut()
                    .set_plain_text(QString::from(&model.plain_text));
                self.as_mut().rust_mut().model = Some(model);
                self.sync();
            }
            Err(error) => self.set_status(QString::from(error)),
        }
    }

    fn sync(mut self: Pin<&mut Self>) {
        if let Some(model) = &self.rust().model {
            let values = (
                model.current_text(),
                model.status.clone(),
                model.current_utf16(),
                model.current_utf16_end(),
                model.speed,
                model.playing,
                if self.rust().preparing {
                    self.rust().comfort.preparation_left(self.now()).max(1) as u32
                } else {
                    (f64::from(model.delay_ms()) / self.rust().comfort.fraction(self.now())).round()
                        as u32
                },
            );
            let extras = (
                model.chapter_index(),
                model.follow_notes,
                model.feedback_enabled,
                model.feedback_chapter(),
            );
            let recovery = model.recovery_description().unwrap_or_default();
            let recovery_needed = model.needs_recovery();
            self.as_mut().set_chapter_index(extras.0);
            self.as_mut().set_follow_notes(extras.1);
            self.as_mut().set_feedback_enabled(extras.2);
            self.as_mut().set_feedback_chapter(QString::from(extras.3));
            self.as_mut().set_recovery_needed(recovery_needed);
            self.as_mut().set_recovery(QString::from(recovery));
            self.as_mut().set_word(QString::from(values.0));
            self.as_mut().set_status(QString::from(values.1));
            self.as_mut()
                .set_position(i32::try_from(values.2).unwrap_or(i32::MAX));
            self.as_mut()
                .set_end_position(i32::try_from(values.3).unwrap_or(i32::MAX));
            self.as_mut()
                .set_speed(i32::try_from(values.4).unwrap_or(350));
            self.as_mut()
                .set_delay_ms(i32::try_from(values.6).unwrap_or(1000));
            self.set_playing(values.5);
        }
    }

    pub fn chapter_position(&self, index: i32) -> i32 {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.rust().model.as_ref()?.chapter_position(index))
            .and_then(|position| i32::try_from(position).ok())
            .unwrap_or(-1)
    }
    pub fn jump_chapter(mut self: Pin<&mut Self>, index: i32) {
        self.as_mut().finish_exposure();
        self.as_mut().rust_mut().comfort = Comfort::default();
        self.as_mut().set_preparing(false);
        if let Some(model) = &mut self.as_mut().rust_mut().model
            && let Ok(index) = usize::try_from(index)
        {
            model.jump_chapter(index);
        }
        self.as_mut().sync();
        self.persist();
    }
    pub fn return_to_text(mut self: Pin<&mut Self>) {
        self.as_mut().finish_exposure();
        self.as_mut().rust_mut().comfort = Comfort::default();
        self.as_mut().set_preparing(false);
        if let Some(model) = &mut self.as_mut().rust_mut().model {
            model.return_to_text();
        }
        self.as_mut().sync();
        self.persist();
    }
    pub fn set_options(mut self: Pin<&mut Self>, notes: bool, feedback: bool) {
        if let Some(model) = &mut self.as_mut().rust_mut().model {
            model.follow_notes = notes;
            model.feedback_enabled = feedback;
        }
        self.as_mut().sync();
        self.persist();
    }
    pub fn finish_feedback(
        mut self: Pin<&mut Self>,
        pace: &QString,
        understanding: &QString,
        accept_speed: bool,
    ) {
        if let Some(model) = &mut self.as_mut().rust_mut().model {
            model.finish_feedback(&pace.to_string(), &understanding.to_string(), accept_speed);
        }
        if let Some(model) = &self.rust().model {
            let history = QString::from(model.history_text());
            self.as_mut().set_history(history);
        }
        self.as_mut().sync();
        self.persist();
    }
    pub fn clear_history(mut self: Pin<&mut Self>) {
        if let Some(model) = &mut self.as_mut().rust_mut().model {
            model.clear_history();
        }
        if let Some(model) = &self.rust().model {
            let history = QString::from(model.history_text());
            self.as_mut().set_history(history);
        }
        self.as_mut().sync();
        self.persist();
    }
    pub fn select(mut self: Pin<&mut Self>, position: i32) {
        self.as_mut().finish_exposure();
        self.as_mut().rust_mut().comfort = Comfort::default();
        self.as_mut().set_preparing(false);
        if let Some(model) = &mut self.as_mut().rust_mut().model {
            model.select_utf16(usize::try_from(position).unwrap_or(0));
        }
        self.as_mut().sync();
        self.persist();
    }
    pub fn play(mut self: Pin<&mut Self>) {
        if self.rust().playing {
            return;
        }
        let now = self.now();
        self.as_mut().rust_mut().comfort.start(now);
        if let Some(model) = &mut self.as_mut().rust_mut().model {
            model.start();
        }
        let preparing = self.rust().model.as_ref().is_some_and(|m| m.playing)
            && self.rust().comfort.preparation_left(now) > 0;
        self.as_mut().set_preparing(preparing);
        if !preparing {
            self.as_mut().begin_exposure();
        }
        self.sync();
    }
    pub fn pause(mut self: Pin<&mut Self>) {
        self.as_mut().finish_exposure();
        let now = self.now();
        self.as_mut().rust_mut().comfort.pause(now);
        self.as_mut().set_preparing(false);
        if let Some(model) = &mut self.as_mut().rust_mut().model {
            model.pause();
        }
        self.as_mut().sync();
        self.persist();
    }
    pub fn tick(mut self: Pin<&mut Self>) {
        if self.rust().preparing {
            if self.rust().comfort.preparation_left(self.now()) == 0 {
                self.as_mut().set_preparing(false);
                self.as_mut().begin_exposure();
            }
            self.sync();
            return;
        }
        self.as_mut().finish_exposure();
        if let Some(model) = &mut self.as_mut().rust_mut().model {
            model.step();
        }
        self.as_mut().sync();
        if !self.rust().playing {
            let now = self.now();
            self.as_mut().rust_mut().comfort.pause(now);
            self.persist();
        } else {
            self.begin_exposure();
        }
    }
    pub fn change_speed(mut self: Pin<&mut Self>, value: i32) {
        if let Some(model) = &mut self.as_mut().rust_mut().model {
            model.speed = u32::try_from(value.clamp(60, 1200)).unwrap_or(350);
        }
        self.as_mut().sync();
        self.persist();
    }
    pub fn confirm_recovery(mut self: Pin<&mut Self>) {
        if let Some(model) = &mut self.as_mut().rust_mut().model
            && let Err(error) = model.confirm_recovery()
        {
            model.status = error;
        }
        self.as_mut().sync();
        self.persist();
    }
    pub fn confirm_selection(mut self: Pin<&mut Self>) {
        if let Some(model) = &mut self.as_mut().rust_mut().model
            && let Err(error) = model.confirm_current_location()
        {
            model.status = error;
        }
        self.as_mut().sync();
        self.persist();
    }
    pub fn persist(mut self: Pin<&mut Self>) {
        if let Some(model) = &self.rust().model
            && let Err(error) = model.save(&self.rust().state_path)
        {
            self.as_mut().set_status(QString::from(error));
        }
    }
}
