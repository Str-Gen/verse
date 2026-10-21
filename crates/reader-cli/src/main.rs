use reader_core::{
    ImportOptions, export_epub, import_epub,
    location::{LocalStore, recover_location},
    verify_sources,
};
use std::{error::Error, path::PathBuf};

fn main() {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("reader_core=info")),
        )
        .with_writer(std::io::stderr)
        .init();
    if let Err(error) = run() {
        tracing::error!(code = "command_failed", reason = %error, "Verse could not complete the request");
        std::process::exit(1);
    }
}
fn run() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "help".into());
    if command == "help" || command == "--help" {
        println!(
            "reader-core export --epub PATH --output PATH [--store PATH]\nreader-core demo --output PATH [--store PATH]\nreader-core check --epub PATH [--store PATH]\nreader-core save --epub PATH --piece ID [--store PATH]\nreader-core restore --epub PATH [--store PATH]\nDefault store: workspace-local scratch/store. check prints counts only."
        );
        return Ok(());
    }
    let mut epub = None;
    let mut output = None;
    let mut piece = None;
    let mut store_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scratch/store");
    let options = ImportOptions::default();
    while let Some(flag) = args.next() {
        let value = args.next().ok_or("flag requires a value")?;
        match flag.as_str() {
            "--epub" => epub = Some(PathBuf::from(value)),
            "--output" => output = Some(PathBuf::from(value)),
            "--store" => store_path = PathBuf::from(value),
            "--piece" => piece = Some(value),
            _ => return Err("unknown flag".into()),
        }
    }
    if command == "demo" {
        epub = Some(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../reader-core/fixtures/complex.epub"),
        );
    }
    let epub = epub.ok_or("--epub PATH is required")?;
    let store = LocalStore::open(store_path)?;
    let book_id = store.identity_for(&epub)?;
    if matches!(command.as_str(), "demo" | "export") {
        let output = output.ok_or("--output PATH is required")?;
        let summary = export_epub(&epub, &book_id, &options, &output)?;
        println!(
            "Exported schema_version=1 documents={} pieces={} chapters={} notes={}",
            summary.documents, summary.pieces, summary.chapters, summary.notes
        );
        return Ok(());
    }
    let book = import_epub(&epub, &book_id, &options)?;
    match command.as_str() {
        "check" => {
            let checked = verify_sources(&epub, &book, &options.limits)?;
            let ornaments = book
                .image_evidence
                .iter()
                .filter(|e| e.classification == "ornament")
                .count();
            let mirrored = book
                .image_evidence
                .iter()
                .filter(|e| e.mirrored_or_identical_assets > 1)
                .count();
            let image_occurrences: usize = book
                .image_evidence
                .iter()
                .map(|e| e.exact_occurrences)
                .sum();
            let unsupported = book
                .pieces
                .iter()
                .filter(|p| p.kind == "unsupported")
                .count();
            println!(
                "Verified source pieces={checked} image_occurrences={image_occurrences} unsupported={unsupported} documents={} chapters={} notes={} raster_assets={} ornaments={ornaments} mirrored_group_assets={mirrored} diagnostics={}",
                book.documents.len(),
                book.chapters.len(),
                book.notes.len(),
                book.image_evidence.len(),
                book.diagnostics.len()
            );
        }
        "save" => {
            store.save_location(&book, &piece.ok_or("--piece ID required")?)?;
            println!("Saved source location.");
        }
        "restore" => {
            let saved = store
                .load_location(&book.book_id)?
                .ok_or("no saved location")?;
            // IDs/status only: chapter titles and saved passage text are private.
            match recover_location(&book, &saved) {
                reader_core::location::Recovery::Exact { piece_id } => println!("exact {piece_id}"),
                reader_core::location::Recovery::ProposePassage { piece_id } => {
                    println!("propose_passage {piece_id}; confirmation required")
                }
                reader_core::location::Recovery::ProposeChapter { piece_id, .. } => {
                    println!("propose_chapter {piece_id}; confirmation required")
                }
                reader_core::location::Recovery::Preserve { reason } => {
                    println!("preserve {reason}")
                }
            }
        }
        _ => return Err("unknown command".into()),
    }
    Ok(())
}
