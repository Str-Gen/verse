use crate::model::Model;
use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
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
        type Reader = super::ReaderRust;

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
    model: Option<Model>,
    state_path: PathBuf,
}

fn argument(name: &str) -> Option<String> {
    let args: Vec<_> = std::env::args().collect();
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

impl ffi::Reader {
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
                let previous_error = self
                    .rust()
                    .model
                    .as_ref()
                    .and_then(|current| current.save(&self.rust().state_path).err());
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
                model.delay_ms(),
            );
            let recovery = model.recovery_description().unwrap_or_default();
            let recovery_needed = model.needs_recovery();
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

    pub fn select(mut self: Pin<&mut Self>, position: i32) {
        if let Some(model) = &mut self.as_mut().rust_mut().model {
            model.select_utf16(usize::try_from(position).unwrap_or(0));
        }
        self.as_mut().sync();
        self.persist();
    }
    pub fn play(mut self: Pin<&mut Self>) {
        if let Some(model) = &mut self.as_mut().rust_mut().model {
            model.start();
        }
        self.sync();
    }
    pub fn pause(mut self: Pin<&mut Self>) {
        if let Some(model) = &mut self.as_mut().rust_mut().model {
            model.pause();
        }
        self.as_mut().sync();
        self.persist();
    }
    pub fn tick(mut self: Pin<&mut Self>) {
        if let Some(model) = &mut self.as_mut().rust_mut().model {
            model.step();
        }
        self.as_mut().sync();
        if !self.rust().playing {
            self.persist();
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
