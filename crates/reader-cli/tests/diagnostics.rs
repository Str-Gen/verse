use std::{error::Error, io::Write, path::PathBuf};

#[test]
fn duplicate_ids_emit_structured_warnings_without_modifying_the_epub() -> Result<(), Box<dyn Error>>
{
    let root = std::env::temp_dir().join(format!("verse-cli-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root)?;
    let path = root.join("duplicate.epub");
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../reader-core/fixtures/complex.epub");
    let mut source = zip::ZipArchive::new(std::fs::File::open(fixture)?)?;
    let mut output = zip::ZipWriter::new(std::fs::File::create(&path)?);
    for index in 0..source.len() {
        let mut entry = source.by_index(index)?;
        output.start_file(entry.name(), zip::write::SimpleFileOptions::default())?;
        if entry.name() == "EPUB/a.xhtml" {
            output.write_all(br#"<html xmlns="http://www.w3.org/1999/xhtml"><body><h1 id="same">One</h1><p id="same">Two</p></body></html>"#)?;
        } else {
            std::io::copy(&mut entry, &mut output)?;
        }
    }
    output.finish()?;
    let original = std::fs::read(&path)?;
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_reader-core"))
        .args(["check", "--epub"])
        .arg(&path)
        .arg("--store")
        .arg(root.join("store"))
        .env("RUST_LOG", "reader_core=warn")
        .output()?;
    assert!(result.status.success());
    let events: Vec<serde_json::Value> = String::from_utf8(result.stderr)?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let warning = events
        .iter()
        .find(|event| event["fields"]["code"] == "duplicate_xhtml_id")
        .ok_or("duplicate warning missing")?;
    assert_eq!(warning["level"], "WARN");
    assert_eq!(warning["fields"]["document"], "EPUB/a.xhtml");
    assert_eq!(warning["fields"]["id"], "same");
    assert_eq!(warning["fields"]["epub"], path.to_string_lossy().as_ref());
    assert_eq!(std::fs::read(&path)?, original);
    std::fs::remove_dir_all(root)?;
    Ok(())
}
