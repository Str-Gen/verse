use super::*;
use proptest::prelude::*;

proptest! {
    #[test]
    fn local_paths_round_trip_without_becoming_uri_syntax(name in "[^/\x00]{1,80}") {
        let path = format!("/tmp/verse/{name}");
        let uri = local_image_uri(&path, Path::new("/unused")).map_err(TestCaseError::fail)?;
        prop_assert!(uri.starts_with("file:///tmp/verse/"));
        prop_assert!(!uri.contains(['?', '#', ' ', '\n']));
        prop_assert_eq!(local_image_uri(&uri, Path::new("/unused")).map_err(TestCaseError::fail)?, uri);
    }

    #[test]
    fn out_of_range_chapters_preserve_selection(extra in 0usize..=usize::MAX / 2) {
        let mut model = Model::from_json(include_str!("../fixtures/demo.json"), Path::new("fixtures"))
            .map_err(TestCaseError::fail)?;
        model.select_utf16(usize::MAX);
        let selected = model.current;
        prop_assert_eq!(selected, model.book.pieces.len() - 1);
        model.jump_chapter(model.chapter_positions.len() + extra);
        prop_assert_eq!(model.current, selected);
        prop_assert!(!model.playing);
    }
}

#[test]
fn malformed_file_uris_are_rejected() {
    for input in [
        "file:///tmp/%",
        "file:///tmp/%0",
        "file:///tmp/%GG",
        "file:///tmp/%FF",
        "file:///tmp/%00",
        "file://remote/tmp/a",
        "file:relative",
        "file:///tmp/a?query",
        "file:///tmp/a#fragment",
        "https://example.com/a",
        "//example.com/a",
        "javascript:alert(1)",
    ] {
        assert!(
            local_image_uri(input, Path::new("/tmp")).is_err(),
            "accepted {input}"
        );
    }
}

#[test]
fn malformed_books_fail_before_rendering() -> Result<(), Box<dyn std::error::Error>> {
    let original: Value = serde_json::from_str(include_str!("../fixtures/demo.json"))?;
    for (pointer, value) in [
        ("/schema_version", json!(2)),
        ("/pieces", json!([])),
        ("/pieces/0/block_id", json!("missing")),
        ("/pieces/0/kind", json!("executable")),
        ("/chapters/0/start_piece", json!("missing")),
        ("/documents/0/blocks/0/kind", json!("executable")),
        (
            "/documents/0/blocks/0/runs/0/sources/0/offset",
            json!(u64::MAX),
        ),
    ] {
        let mut invalid = original.clone();
        *invalid
            .pointer_mut(pointer)
            .ok_or("Missing fixture field")? = value;
        assert!(
            Model::from_json(&invalid.to_string(), Path::new("/tmp")).is_err(),
            "accepted {pointer}"
        );
    }
    Ok(())
}

#[test]
fn corrupt_resume_file_is_reported_without_overwriting_it() -> Result<(), Box<dyn std::error::Error>>
{
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("state.json");
    let bytes = b"{\"books\": truncated";
    fs::write(&path, bytes)?;
    let mut model = Model::from_json(include_str!("../fixtures/demo.json"), Path::new("fixtures"))?;
    assert!(model.restore(&path).is_err());
    assert_eq!(fs::read(&path)?, bytes);
    assert!(!model.playing);
    Ok(())
}
