//! Narrow adapter over version-1 normalized book JSON. No EPUB or publisher code executes here.
mod theme;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashSet,
    env, fs,
    io::Read,
    path::{Path, PathBuf},
    sync::Mutex,
};

const MAX_BOOK_BYTES: u64 = 64 * 1024 * 1024;
const MAX_STATE_BYTES: u64 = 1024 * 1024;

struct Store {
    root: PathBuf,
    book: Mutex<reader_core::Book>,
    self_test: bool,
}

#[derive(Serialize)]
struct Boot {
    book: Value,
    state: Option<Value>,
    self_test: bool,
    theme_test: bool,
}

#[tauri::command]
fn read_theme() -> std::collections::BTreeMap<String, String> {
    theme::read()
}

fn read_json(path: &Path, limit: u64) -> Result<Value, String> {
    let file = fs::File::open(path).map_err(|_| "Could not open local JSON file.")?;
    if file
        .metadata()
        .map_err(|_| "Could not inspect local JSON file.")?
        .len()
        > limit
    {
        return Err("Local JSON file exceeds the size limit.".into());
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Could not read local JSON file.")?;
    if bytes.len() as u64 > limit {
        return Err("Local JSON file exceeds the size limit.".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "Local file is not valid JSON.".into())
}

fn nonempty(value: &Value, key: &str) -> bool {
    value
        .get(key)
        .and_then(Value::as_str)
        .is_some_and(|s| !s.is_empty())
}

#[derive(Deserialize)]
struct ContractBook {
    documents: Vec<Document>,
    pieces: Vec<Piece>,
    chapters: Vec<Chapter>,
}
#[derive(Deserialize)]
struct Document {
    id: String,
    blocks: Vec<Block>,
}
#[derive(Deserialize)]
struct Block {
    id: String,
    kind: String,
    runs: Vec<Run>,
}
#[derive(Deserialize)]
struct Run {
    text: String,
    sources: Vec<Span>,
}
#[derive(Deserialize)]
struct Span {
    document: String,
    node: String,
    offset: u64,
    length: u64,
}
#[derive(Deserialize)]
struct Piece {
    id: String,
    text: String,
    block_id: String,
    sources: Vec<Span>,
}
#[derive(Deserialize)]
struct Chapter {
    id: String,
    title: String,
    start_piece: String,
    end_piece_exclusive: Option<String>,
}

fn valid_spans(spans: &[Span], documents: &HashSet<&str>) -> bool {
    spans.iter().all(|s| {
        documents.contains(s.document.as_str())
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
    let parsed: ContractBook = serde_json::from_value(book.clone()).map_err(|_| invalid())?;
    let documents: HashSet<_> = parsed.documents.iter().map(|d| d.id.as_str()).collect();
    if documents.len() != parsed.documents.len() || documents.contains("") {
        return Err(invalid());
    }
    let mut blocks = HashSet::new();
    for document in &parsed.documents {
        for block in &document.blocks {
            if block.id.is_empty() || block.kind.is_empty() || !blocks.insert(block.id.as_str()) {
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
            || !pieces.insert(piece.id.as_str())
            || !blocks.contains(piece.block_id.as_str())
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
            || !chapters.insert(chapter.id.as_str())
            || chapter.title.len() as u64 > MAX_BOOK_BYTES
            || !pieces.contains(chapter.start_piece.as_str())
            || chapter
                .end_piece_exclusive
                .as_ref()
                .is_some_and(|id| !pieces.contains(id.as_str()))
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
    validate_book(&book)?;
    let parsed: reader_core::Book =
        reader_core::session::book_from_value(book.clone()).map_err(|e| e.to_string())?;
    let resume =
        reader_core::session::restore(&parsed, saved.as_ref()).map_err(|e| e.to_string())?;
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
    let result = serde_json::json!({"book": &parsed, "resume": resume, "normal": normal, "notes": notes, "note_plans": note_plans});
    *store
        .book
        .lock()
        .map_err(|_| "Book state is unavailable.")? = parsed;
    Ok(result)
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
    let state_path = store.root.join("state.json");
    let state = if state_path.exists() {
        {
            let state: Value =
                reader_core::location::read_json(&state_path).map_err(|e| e.to_string())?;
            if !state.get("books").is_some_and(Value::is_object) {
                return Err("Invalid saved books; existing state preserved.".into());
            }
            Some(state)
        }
    } else {
        None
    };
    Ok(Boot {
        book,
        state,
        self_test: store.self_test,
        theme_test: store.self_test && env::var_os("READER_THEME_TEST").is_some(),
    })
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command arguments are owned IPC values and managed handles"
)]
fn import_book(json: String, store: tauri::State<'_, Store>) -> Result<Value, String> {
    if json.len() as u64 > MAX_BOOK_BYTES {
        return Err("Book JSON exceeds the size limit.".into());
    }
    let book: Value = serde_json::from_str(&json).map_err(|_| "Book is not valid JSON.")?;
    validate_book(&book)?;
    let mut current = store
        .book
        .lock()
        .map_err(|_| "Book state is unavailable.")?;
    write_json(&store.root, "book.json", &book)?;
    *current = reader_core::session::book_from_value(book.clone()).map_err(|e| e.to_string())?;
    Ok(book)
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command arguments are owned IPC values and managed handles"
)]
fn save_state(
    mut state: Value,
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
    let guard = store
        .book
        .lock()
        .map_err(|_| "Book state is unavailable.")?;
    let book = &*guard;
    if book.book_id != book_id {
        return Err("Book changed before save.".into());
    }
    let record = reader_core::session::capture(book, at, &state["books"][&book_id])
        .map_err(|e| e.to_string())?;
    state["books"][&book_id] = record.clone();
    write_json(&store.root, "state.json", &state)?;
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
    let book = if self_test {
        serde_json::from_str(include_str!("../../web/demo.json"))
            .map_err(|_| "Bundled demo is invalid.")?
    } else if let Some(path) = book_path {
        read_json(&path, MAX_BOOK_BYTES)?
    } else if root.join("book.json").exists() {
        read_json(&root.join("book.json"), MAX_BOOK_BYTES)?
    } else {
        serde_json::from_str(include_str!("../../web/demo.json"))
            .map_err(|_| "Bundled demo is invalid.")?
    };
    validate_book(&book)?;
    write_json(&root, "book.json", &book)?;
    Ok(Store {
        root,
        book: Mutex::new(reader_core::session::book_from_value(book).map_err(|e| e.to_string())?),
        self_test,
    })
}

fn main() {
    let store = match initialize() {
        Ok(store) => store,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let webview_data = store.root.join("webview");
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
            read_theme,
            import_book,
            save_state,
            test_result
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
