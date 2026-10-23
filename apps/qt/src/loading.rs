use crate::model::Model;
use reader_core::persistence::BookLock;
use std::{
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver},
};

pub(crate) struct LoadedBook {
    pub model: Model,
    pub lock: Option<BookLock>,
    pub path: PathBuf,
}

pub(crate) fn start(
    path: PathBuf,
    state: PathBuf,
    held_book: Option<String>,
) -> Result<Receiver<Result<LoadedBook, String>>, String> {
    let (send, receive) = mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("book-loader".into())
        .spawn(move || {
            let result = load(path, &state, held_book.as_deref());
            // A closed receiver means the window has gone away; dropping releases the book lease.
            let _ = send.send(result);
        })
        .map_err(|error| format!("Cannot start book loader: {error}"))?;
    Ok(receive)
}

fn load(path: PathBuf, state: &Path, held_book: Option<&str>) -> Result<LoadedBook, String> {
    let imports = std::env::var_os("READER_IMPORT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../scripts/.local"))
        });
    let mut lock = match Model::epub_identity(&path, &imports)? {
        Some(id) if Some(id.as_str()) != held_book => Some(BookLock::acquire(&id)?),
        _ => None,
    };
    let mut model = Model::open(&path, &imports)?;
    if lock.is_none() && Some(model.book_id()) != held_book {
        lock = Some(BookLock::acquire(model.book_id())?);
    }
    if let Err(error) = model.restore(state) {
        model.status = error;
    }
    Ok(LoadedBook { model, lock, path })
}
