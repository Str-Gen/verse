//! Narrow adapter over version-1 normalized book JSON. No EPUB or publisher code executes here.
mod theme;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashSet,
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};

const MAX_BOOK_BYTES: u64 = 64 * 1024 * 1024;
const MAX_STATE_BYTES: u64 = 1024 * 1024;

struct Store {
    root: PathBuf,
    book: Mutex<reader_core::Book>,
    disk: Mutex<reader_core::book_store::DiskBook>,
    book_lock: Mutex<reader_core::persistence::BookLock>,
    self_test: bool,
    startup_warning: Option<String>,
    upload: Mutex<Option<(fs::File, PathBuf, u64)>>,
    vocabulary: Mutex<Option<reader_core::vocabulary::Vocabulary>>,
}

#[derive(Serialize)]
struct Boot {
    book: Value,
    state: Option<Value>,
    self_test: bool,
    theme_test: bool,
    warning: Option<String>,
    memory_test: bool,
}

#[tauri::command]
fn read_theme() -> std::collections::BTreeMap<String, String> {
    theme::read()
}

fn nonempty(value: &Value, key: &str) -> bool {
    value
        .get(key)
        .and_then(Value::as_str)
        .is_some_and(|s| !s.is_empty())
}

#[derive(Deserialize)]
struct ContractBook<'a> {
    #[serde(borrow)]
    documents: Vec<Document<'a>>,
    #[serde(borrow)]
    pieces: Vec<Piece<'a>>,
    #[serde(borrow)]
    chapters: Vec<Chapter<'a>>,
}
#[derive(Deserialize)]
struct Document<'a> {
    id: &'a str,
    #[serde(borrow)]
    blocks: Vec<Block<'a>>,
}
#[derive(Deserialize)]
struct Block<'a> {
    id: &'a str,
    kind: &'a str,
    #[serde(borrow)]
    runs: Vec<Run<'a>>,
}
#[derive(Deserialize)]
struct Run<'a> {
    text: &'a str,
    #[serde(borrow)]
    sources: Vec<Span<'a>>,
}
#[derive(Deserialize)]
struct Span<'a> {
    document: &'a str,
    node: &'a str,
    offset: u64,
    length: u64,
}
#[derive(Deserialize)]
struct Piece<'a> {
    id: &'a str,
    text: &'a str,
    block_id: &'a str,
    #[serde(borrow)]
    sources: Vec<Span<'a>>,
}
#[derive(Deserialize)]
struct Chapter<'a> {
    id: &'a str,
    title: &'a str,
    start_piece: &'a str,
    end_piece_exclusive: Option<&'a str>,
}

fn valid_spans(spans: &[Span<'_>], documents: &HashSet<&str>) -> bool {
    spans.iter().all(|s| {
        documents.contains(s.document)
            && !s.node.is_empty()
            && s.offset
                .checked_add(s.length)
                .is_some_and(|end| end <= 9_007_199_254_740_991)
    })
}

fn validate_book(book: &Value) -> Result<(), String> {
    let invalid = || "Invalid version-1 normalized book structure or source references.".to_owned();
    if book.get("schema_version").and_then(Value::as_u64) != Some(1)
        || !nonempty(book, "book_id")
        || !nonempty(book, "revision")
        || !book.get("title").is_some_and(Value::is_string)
        || !book.get("notes").is_some_and(Value::is_array)
    {
        return Err(invalid());
    }
    let parsed = ContractBook::deserialize(book).map_err(|_| invalid())?;
    let documents: HashSet<_> = parsed.documents.iter().map(|d| d.id).collect();
    if documents.len() != parsed.documents.len() || documents.contains("") {
        return Err(invalid());
    }
    let mut blocks = HashSet::new();
    for document in &parsed.documents {
        for block in &document.blocks {
            if block.id.is_empty() || block.kind.is_empty() || !blocks.insert(block.id) {
                return Err(invalid());
            }
            for run in &block.runs {
                let count = run
                    .sources
                    .iter()
                    .try_fold(0u64, |sum, s| sum.checked_add(s.length));
                if !valid_spans(&run.sources, &documents)
                    || count != Some(run.text.chars().count() as u64)
                {
                    return Err(invalid());
                }
            }
        }
    }
    let mut pieces = HashSet::new();
    for piece in &parsed.pieces {
        if piece.id.is_empty()
            || !pieces.insert(piece.id)
            || !blocks.contains(piece.block_id)
            || !valid_spans(&piece.sources, &documents)
            || piece.text.len() as u64 > MAX_BOOK_BYTES
        {
            return Err(invalid());
        }
    }
    if pieces.is_empty() {
        return Err(invalid());
    }
    let mut chapters = HashSet::new();
    for chapter in &parsed.chapters {
        if chapter.id.is_empty()
            || !chapters.insert(chapter.id)
            || chapter.title.len() as u64 > MAX_BOOK_BYTES
            || !pieces.contains(chapter.start_piece)
            || chapter
                .end_piece_exclusive
                .is_some_and(|id| !pieces.contains(id))
        {
            return Err(invalid());
        }
    }
    Ok(())
}

fn write_json(root: &Path, name: &str, value: &Value) -> Result<(), String> {
    reader_core::location::write_json_with_limit(
        &root.join(name),
        value,
        if name == "book.json" {
            MAX_BOOK_BYTES
        } else {
            MAX_STATE_BYTES
        },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
#[expect(clippy::needless_pass_by_value, reason = "owned IPC arguments")]
fn prepare_session(
    book: Value,
    saved: Option<Value>,
    store: tauri::State<'_, Store>,
) -> Result<Value, String> {
    let disk = store.disk.lock().map_err(|_| "Book storage unavailable")?;
    if book["book_id"].as_str() != Some(disk.metadata().book_id.as_str()) {
        return Err("Book changed before preparing the session".into());
    }
    let durable =
        reader_core::persistence::load_record_for_book(&store.root.join("state.json"), &disk)
            .map_err(|e| e.to_string())?;
    let resume = disk
        .restore(durable.as_ref().or(saved.as_ref()))
        .map_err(|e| e.to_string())?;
    let window = disk.load_chapter(resume.at).map_err(|e| e.to_string())?;
    let local_at = resume.at.saturating_sub(window.start);
    let mut result = prepare_window(&store, &disk, window)?;
    result["resume"] = serde_json::json!({"kind": resume.kind, "at": local_at});
    result["preferences"] = durable.or(saved).unwrap_or(Value::Null);
    Ok(result)
}

fn prepare_window(
    store: &Store,
    disk: &reader_core::book_store::DiskBook,
    window: reader_core::book_store::BookWindow,
) -> Result<Value, String> {
    let parsed = window.book;
    let normal = reader_core::session::plan(&parsed.pieces, &parsed.chapters, false);
    let notes = reader_core::session::plan(&parsed.pieces, &parsed.chapters, true);
    let note_plans: std::collections::BTreeMap<_, _> = parsed
        .notes
        .iter()
        .map(|note| {
            (
                &note.id,
                reader_core::session::plan(&note.pieces, &[], false),
            )
        })
        .collect();
    let mut vocabulary = store
        .vocabulary
        .lock()
        .map_err(|_| "Vocabulary state unavailable")?;
    let vocabulary_error = vocabulary.as_mut().and_then(|worker| worker.request(&serde_json::json!({
        "action":"open", "book":{"book_id":parsed.book_id,"title":parsed.title,"documents":parsed.documents,"pieces":parsed.pieces}
    })).err());
    let chapters: Vec<_> = disk
        .metadata()
        .chapters
        .iter()
        .zip(disk.chapter_starts())
        .map(|(c, at)| serde_json::json!({"id":c.id,"title":c.title,"at":at}))
        .collect();
    let result = serde_json::json!({
        "comfort": {"preparation":reader_core::experience::PREPARATION_MS,"ramp":reader_core::experience::RAMP_MS,"brief_pause":reader_core::experience::BRIEF_PAUSE_MS},
        "vocabulary_error": vocabulary_error, "book": &parsed, "normal": normal, "notes": notes, "note_plans": note_plans,
        "window": {"start":window.start,"total":window.total,"chapters":chapters,"image_dimensions":window.image_dimensions}
    });
    validate_book(&result["book"])?;
    *store.book.lock().map_err(|_| "Book state unavailable")? = parsed;
    Ok(result)
}

#[tauri::command]
#[expect(clippy::needless_pass_by_value, reason = "owned IPC arguments")]
fn load_window(
    at: usize,
    book_id: String,
    store: tauri::State<'_, Store>,
) -> Result<Value, String> {
    let disk = store.disk.lock().map_err(|_| "Book storage unavailable")?;
    if disk.metadata().book_id != book_id {
        return Err("Book changed before navigation".into());
    }
    let window = disk.load_chapter(at).map_err(|e| e.to_string())?;
    prepare_window(&store, &disk, window)
}

#[tauri::command]
#[expect(clippy::needless_pass_by_value, reason = "owned IPC arguments")]
fn image_resources(
    at: usize,
    book_id: String,
    store: tauri::State<'_, Store>,
) -> Result<Vec<reader_core::Resource>, String> {
    let disk = store.disk.lock().map_err(|_| "Book storage unavailable")?;
    if disk.metadata().book_id != book_id {
        return Err("Book changed before image prefetch".into());
    }
    disk.image_resources(at).map_err(|e| e.to_string())
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command arguments are owned IPC values and managed handles"
)]
fn boot(store: tauri::State<'_, Store>) -> Result<Boot, String> {
    let book = serde_json::to_value(
        &*store
            .book
            .lock()
            .map_err(|_| "Book state is unavailable.")?,
    )
    .map_err(|e| e.to_string())?;
    let id = book["book_id"].as_str().ok_or("Missing book identity")?;
    let state = reader_core::persistence::load_record(&store.root.join("state.json"), id)
        .map_err(|e| e.to_string())?
        .map(|record| serde_json::json!({"books":{id:record}}));
    Ok(Boot {
        book,
        state,
        self_test: store.self_test,
        theme_test: store.self_test && env::var_os("READER_THEME_TEST").is_some(),
        warning: store.startup_warning.clone(),
        memory_test: store.self_test && env::var_os("READER_MEMORY_TEST").is_some(),
    })
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command arguments are owned IPC values and managed handles"
)]
fn import_book(json: String, store: tauri::State<'_, Store>) -> Result<Value, String> {
    if json.len() as u64 > MAX_BOOK_BYTES {
        return Err("Book JSON exceeds the size limit".into());
    }
    let path = unique_import_path(&store.root)?;
    fs::write(&path, json.as_bytes()).map_err(|e| e.to_string())?;
    open_disk(&store, &path)
}

fn unique_import_path(root: &Path) -> Result<PathBuf, String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    Ok(root.join(format!("import-{}-{stamp}.json", std::process::id())))
}

fn open_disk(store: &Store, path: &Path) -> Result<Value, String> {
    let next = reader_core::book_store::DiskBook::open(path).map_err(|e| e.to_string())?;
    let window = next.load_chapter(0).map_err(|e| e.to_string())?;
    let mut lock = store
        .book_lock
        .lock()
        .map_err(|_| "Publication lock unavailable")?;
    let next_lock = if lock.book_id() == next.metadata().book_id {
        None
    } else {
        Some(reader_core::persistence::BookLock::acquire(
            &next.metadata().book_id,
        )?)
    };
    let mut disk = store.disk.lock().map_err(|_| "Book storage unavailable")?;
    let result = serde_json::to_value(&window.book).map_err(|e| e.to_string())?;
    validate_book(&result)?;
    reader_core::location::write_json(&store.root.join("last-book.json"), &path)
        .map_err(|e| e.to_string())?;
    *store.book.lock().map_err(|_| "Book state unavailable")? = window.book;
    *disk = next;
    if let Some(next_lock) = next_lock {
        *lock = next_lock;
    }
    Ok(result)
}

#[tauri::command]
#[expect(clippy::needless_pass_by_value, reason = "owned IPC arguments")]
fn upload_book(
    data: Vec<u8>,
    start: bool,
    finish: bool,
    store: tauri::State<'_, Store>,
) -> Result<Option<Value>, String> {
    if data.len() > 65536 {
        return Err("Upload chunk exceeds 64 KiB".into());
    }
    let mut upload = store.upload.lock().map_err(|_| "Upload unavailable")?;
    if start {
        let path = unique_import_path(&store.root)?;
        *upload = Some((fs::File::create(&path).map_err(|e| e.to_string())?, path, 0));
    }
    let (file, path, length) = upload.as_mut().ok_or("No upload in progress")?;
    *length += data.len() as u64;
    if *length > 1024 * 1024 * 1024 {
        *upload = None;
        return Err("Book exceeds 1 GiB".into());
    }
    file.write_all(&data).map_err(|e| e.to_string())?;
    if !finish {
        return Ok(None);
    }
    file.sync_all().map_err(|e| e.to_string())?;
    let path = path.clone();
    *upload = None;
    open_disk(&store, &path).map(Some)
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command arguments are owned IPC values and managed handles"
)]
fn save_state(
    state: Value,
    at: usize,
    book_id: String,
    store: tauri::State<'_, Store>,
) -> Result<Value, String> {
    if !state.is_object()
        || serde_json::to_vec(&state)
            .map_err(|_| "Invalid reading state.")?
            .len() as u64
            > MAX_STATE_BYTES
    {
        return Err("Reading state must be an object within the size limit.".into());
    }
    let disk = store.disk.lock().map_err(|_| "Book storage unavailable")?;
    if disk.metadata().book_id != book_id
        || state["books"][&book_id]["revision"].as_str() != Some(disk.metadata().revision.as_str())
    {
        return Err("Book changed before save".into());
    }
    let record = disk
        .capture(at, &state["books"][&book_id])
        .map_err(|e| e.to_string())?;
    let chapter_at = disk
        .chapter_starts()
        .iter()
        .copied()
        .filter(|start| *start <= at)
        .max()
        .unwrap_or(0);
    let chapter = disk
        .capture(chapter_at, &state["books"][&book_id])
        .map_err(|e| e.to_string())?;
    reader_core::persistence::save_record_for_book(
        &store.root.join("state.json"),
        &disk,
        &record,
        &chapter,
    )
    .map_err(|e| e.to_string())?;
    Ok(record)
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command arguments are owned IPC values and managed handles"
)]
fn test_result(
    result: Value,
    store: tauri::State<'_, Store>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    if !store.self_test {
        return Err("Self-test reporting is disabled in normal sessions.".into());
    }
    write_json(&store.root, "self-test.json", &result)?;
    app.exit(i32::from(
        result.get("passed").and_then(Value::as_bool) != Some(true),
    ));
    Ok(())
}

#[tauri::command]
#[expect(clippy::needless_pass_by_value, reason = "owned IPC arguments")]
fn test_phase(phase: String, store: tauri::State<'_, Store>) -> Result<(), String> {
    if !store.self_test {
        return Err("Test reporting disabled".into());
    }
    write_json(
        &store.root,
        "memory-phase.json",
        &serde_json::json!({"phase":phase}),
    )
}

fn initialize() -> Result<Store, String> {
    let mut book_path = None;
    let mut self_test = false;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--book-json" => {
                book_path = Some(PathBuf::from(
                    args.next().ok_or("--book-json requires a path.")?,
                ));
            }
            "--self-test" => self_test = true,
            _ => return Err("Unknown argument. Use --book-json PATH or --self-test.".into()),
        }
    }
    let owned = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("Invalid build path.")?
        .to_path_buf();
    let normal_root =
        env::var_os("READER_SCRATCH").map_or_else(|| owned.join(".local"), PathBuf::from);
    let root = if self_test {
        normal_root.join("self-test")
    } else {
        normal_root
    };
    fs::create_dir_all(&root).map_err(|_| "Could not create isolated reader storage.")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
            .map_err(|_| "Could not protect isolated reader storage.")?;
    }
    let explicit = book_path.is_some() || self_test;
    let path = if self_test && env::var_os("READER_MEMORY_TEST").is_none() {
        owned.join("web/demo.json")
    } else if let Some(path) = book_path {
        path
    } else {
        reader_core::location::read_json::<PathBuf>(&root.join("last-book.json"))
            .ok()
            .filter(|path| path.is_file())
            .unwrap_or_else(|| {
                if root.join("book.json").is_file() {
                    root.join("book.json")
                } else {
                    owned.join("web/demo.json")
                }
            })
    };
    let (disk, path, startup_warning) = match reader_core::book_store::DiskBook::open(&path) {
        Ok(disk) => (disk, path, None),
        Err(error) if !explicit => {
            let fallback = owned.join("web/demo.json");
            let disk =
                reader_core::book_store::DiskBook::open(&fallback).map_err(|e| e.to_string())?;
            (
                disk,
                fallback,
                Some(format!(
                    "Could not reopen the remembered book: {error}. Open the book again to recover its saved position."
                )),
            )
        }
        Err(error) => return Err(error.to_string()),
    };
    let book_lock = reader_core::persistence::BookLock::acquire(&disk.metadata().book_id)?;
    let window = disk.load_chapter(0).map_err(|e| e.to_string())?;
    reader_core::location::write_json(
        &root.join("last-book.json"),
        &fs::canonicalize(&path).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(Store {
        root,
        book_lock: Mutex::new(book_lock),
        book: Mutex::new(window.book),
        disk: Mutex::new(disk),
        self_test,
        startup_warning,
        upload: Mutex::new(None),
        vocabulary: Mutex::new(None),
    })
}

#[tauri::command]
#[expect(clippy::needless_pass_by_value, reason = "owned IPC arguments")]
fn vocabulary(request: Value, store: tauri::State<'_, Store>) -> Result<Value, String> {
    let mut worker = store
        .vocabulary
        .lock()
        .map_err(|_| "Vocabulary unavailable")?;
    if worker.is_none() {
        let book = store.book.lock().map_err(|_| "Book unavailable")?;
        *worker = Some(reader_core::vocabulary::Vocabulary::open(
            &store.root,
            &book,
        )?);
    }
    worker
        .as_mut()
        .ok_or("Vocabulary unavailable")?
        .request(&request)
}

fn main() {
    let store = match initialize() {
        Ok(store) => store,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let webview_data = store.root.join(format!("webview-{}", std::process::id()));
    if let Err(error) = tauri::Builder::default()
        .setup(move |app| {
            tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::default())
                .title("Verse")
                .inner_size(1040.0, 820.0)
                .min_inner_size(360.0, 480.0)
                .data_directory(webview_data)
                .on_navigation(|url| {
                    url.scheme() == "tauri" || url.host_str() == Some("tauri.localhost")
                })
                .build()?;
            Ok(())
        })
        .manage(store)
        .invoke_handler(tauri::generate_handler![
            boot,
            prepare_session,
            load_window,
            image_resources,
            vocabulary,
            read_theme,
            import_book,
            upload_book,
            save_state,
            test_result,
            test_phase
        ])
        .run(tauri::generate_context!())
    {
        eprintln!("Application could not start: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_demo_and_rejects_unknown_schema() -> Result<(), String> {
        let mut demo: Value = serde_json::from_str(include_str!("../../web/demo.json"))
            .map_err(|_| "Invalid fixture")?;
        validate_book(&demo)?;
        demo["schema_version"] = Value::from(2);
        assert!(validate_book(&demo).is_err());
        Ok(())
    }
    #[test]
    fn rejects_broken_sources_and_duplicate_piece_ids() -> Result<(), String> {
        let original: Value = serde_json::from_str(include_str!("../../web/demo.json"))
            .map_err(|_| "Invalid fixture")?;
        let mut demo = original.clone();
        demo["documents"][0]["blocks"][0]["runs"][0]["sources"][0]["length"] = Value::from(999);
        assert!(validate_book(&demo).is_err());
        let mut demo = original.clone();
        demo["pieces"][0]["sources"][0]["document"] = Value::from("missing");
        assert!(validate_book(&demo).is_err());
        let mut demo = original;
        demo["pieces"][1]["id"] = demo["pieces"][0]["id"].clone();
        assert!(validate_book(&demo).is_err());
        Ok(())
    }
    #[test]
    fn validates_unicode_and_optional_chapter_ends() -> Result<(), String> {
        let mut book = serde_json::json!({
            "schema_version": 1, "book_id": "book", "revision": "revision",
            "title": "", "notes": [],
            "documents": [{"id": "document", "blocks": [{
                "id": "block", "kind": "paragraph", "runs": [{
                    "text": "é\n\"🙂", "sources": [{
                        "document": "document", "node": "node", "offset": 0, "length": 4
                    }]
                }]
            }]}],
            "pieces": [{"id": "piece", "text": "é\n\"🙂", "block_id": "block", "sources": []}],
            "chapters": [{"id": "chapter", "title": "", "start_piece": "piece"}]
        });
        validate_book(&book)?;
        book["chapters"][0]["end_piece_exclusive"] = Value::Null;
        validate_book(&book)?;
        book["chapters"][0]["end_piece_exclusive"] = Value::from("piece");
        validate_book(&book)?;
        for pointer in [
            "/documents/0/id",
            "/documents/0/blocks/0/kind",
            "/documents/0/blocks/0/runs/0/text",
            "/documents/0/blocks/0/runs/0/sources/0/node",
            "/pieces/0/text",
            "/chapters/0/title",
            "/chapters/0/end_piece_exclusive",
        ] {
            let mut invalid = book.clone();
            *invalid
                .pointer_mut(pointer)
                .ok_or("Missing fixture field")? = Value::from(1);
            assert!(validate_book(&invalid).is_err(), "accepted {pointer}");
        }
        book["documents"][0]["blocks"][0]["runs"][0]["sources"][0]["length"] = Value::from(3);
        assert!(validate_book(&book).is_err());
        Ok(())
    }
    #[test]
    fn rejects_missing_identity_and_pieces() -> Result<(), String> {
        let mut demo: Value = serde_json::from_str(include_str!("../../web/demo.json"))
            .map_err(|_| "Invalid fixture")?;
        demo["book_id"] = Value::from("");
        assert!(validate_book(&demo).is_err());
        demo["book_id"] = Value::from("demo");
        demo["pieces"] = Value::Array(Vec::new());
        assert!(validate_book(&demo).is_err());
        Ok(())
    }
}

#[cfg(test)]
mod adversarial;
