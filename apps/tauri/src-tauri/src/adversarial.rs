use super::*;
use proptest::prelude::*;

proptest! {
    #[test]
    fn source_ranges_respect_javascript_integer_precision(
        offset in prop_oneof![0..=9_007_199_254_740_991u64, any::<u64>()],
        length in prop_oneof![0..=1024u64, any::<u64>()],
    ) {
        let spans = [Span { document: "doc", node: "node", offset, length }];
        let expected = u128::from(offset) + u128::from(length) <= 9_007_199_254_740_991;
        prop_assert_eq!(valid_spans(&spans, &HashSet::from(["doc"])), expected);
    }
}

#[test]
fn source_range_precision_boundary_is_inclusive() {
    for (offset, length, accepted) in [
        (9_007_199_254_740_990, 1, true),
        (9_007_199_254_740_991, 0, true),
        (9_007_199_254_740_991, 1, false),
        (u64::MAX, 1, false),
    ] {
        assert_eq!(
            valid_spans(
                &[Span {
                    document: "doc",
                    node: "node",
                    offset,
                    length
                }],
                &HashSet::from(["doc"])
            ),
            accepted
        );
    }
}

#[test]
fn malformed_contract_fields_are_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let original: Value = serde_json::from_str(include_str!("../../web/demo.json"))?;
    for (pointer, value) in [
        ("/revision", Value::from("")),
        ("/notes", Value::Null),
        ("/documents/0/id", Value::from("")),
        ("/documents/0/blocks/0/id", Value::from("")),
        ("/pieces/0/id", Value::from("")),
        ("/pieces/0/block_id", Value::from("missing")),
        ("/pieces/0/sources/0/node", Value::from("")),
        ("/pieces/0/sources/0/offset", Value::from(-1)),
        ("/chapters/0/id", Value::from("")),
        ("/chapters/0/start_piece", Value::from("missing")),
        ("/chapters/0/end_piece_exclusive", Value::from("missing")),
    ] {
        let mut invalid = original.clone();
        *invalid
            .pointer_mut(pointer)
            .ok_or("Missing fixture field")? = value;
        assert!(validate_book(&invalid).is_err(), "accepted {pointer}");
    }
    Ok(())
}

#[test]
fn failed_import_keeps_current_book_and_durable_state() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    let mut book: Value = serde_json::from_str(include_str!("../../web/demo.json"))?;
    book["book_id"] = Value::from(format!("adversarial-{}", root.display()));
    let original = root.join("original.json");
    fs::write(&original, serde_json::to_vec(&book)?)?;
    let disk = reader_core::book_store::DiskBook::open(&original)?;
    let window = disk.load_window(0)?;
    let lock = reader_core::persistence::BookLock::acquire(&window.book.book_id)?;
    let store = Store {
        root: root.to_owned(),
        book: Mutex::new(window.book),
        disk: Mutex::new(disk),
        book_lock: Mutex::new(lock),
        self_test: false,
        startup_warning: None,
        upload: Mutex::new(None),
        vocabulary: Mutex::new(None),
    };
    open_disk(&store, &original)?;
    write_json(root, "state.json", &serde_json::json!({"sentinel": "keep"}))?;
    let last = fs::read(root.join("last-book.json"))?;
    let state = fs::read(root.join("state.json"))?;
    let invalid = root.join("invalid.json");
    for bytes in ["{truncated", "null", "{}", "{\"schema_version\":999}"] {
        fs::write(&invalid, bytes)?;
        assert!(open_disk(&store, &invalid).is_err());
        assert_eq!(fs::read(root.join("last-book.json"))?, last);
        assert_eq!(fs::read(root.join("state.json"))?, state);
        assert_eq!(
            store.book.lock().map_err(|_| "poisoned")?.book_id,
            book["book_id"]
        );
        assert_eq!(
            store
                .disk
                .lock()
                .map_err(|_| "poisoned")?
                .metadata()
                .book_id,
            book["book_id"]
        );
    }
    Ok(())
}
